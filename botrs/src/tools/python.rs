use std::collections::HashMap;
use std::rc::Rc;

use convert_case::{Case, Casing};
use llm_chain::tools::{Describe, Format, Tool, ToolDescription, ToolUseError};
use pyo3::prelude::*;
use pyo3::types::{IntoPyDict, PyDict, PyFloat, PyList, PyTuple};
use serde::{Deserialize, Serialize};
use serde_yaml::Value;

use crate::tools::{invoke_simple_from_toolbox, AdvancedTool, Toolbox};

/// A tool that executes Python code.
#[derive(Default)]
pub struct PythonTool {}

/// The input of the Python tool
#[derive(Serialize, Deserialize)]
pub struct PythonToolInput {
    /// The Python code to execute.
    pub code: String,
}

/// The output of the Python tool
#[derive(Serialize, Deserialize)]
pub struct PythonToolOutput {
    /// The stdout of the executed Python code.
    pub stdout: String,
    /// The stderr output of the Python code execution.
    pub stderr: String,
}

impl Describe for PythonToolInput {
    fn describe() -> Format {
        vec![("code", "The Python code to execute. MANDATORY").into()].into()
    }
}

impl Describe for PythonToolOutput {
    fn describe() -> Format {
        vec![
            ("stdout", "The stdout of the executed Python code.").into(),
            ("stderr", "The stderr output of the Python code execution.").into(),
        ]
        .into()
    }
}

#[pyclass]
#[derive(Default)]
struct Logging {
    output: String,
}

#[pymethods]
impl Logging {
    fn write(&mut self, data: &str) {
        self.output.push_str(data);
    }
}

#[pyclass(unsendable)]
struct ToolsWrapper {
    toolbox: Rc<Toolbox>,
}

impl ToolsWrapper {
    fn new(toolbox: Rc<Toolbox>) -> Self {
        ToolsWrapper { toolbox }
    }
}

#[derive(thiserror::Error, Debug)]
enum PyConversionError {
    #[error("Invalid conversion: {error}")]
    InvalidConversion { error: String },
    #[error("dict key not serializable: {typename}")]
    DictKeyNotSerializable { typename: String },
    #[error("Invalid cast: {typename}")]
    InvalidCast { typename: String },
}

// inspired from https://github.com/mozilla-services/python-canonicaljson-rs/blob/62599b246055a1c8a78e5777acdfe0fd594be3d8/src/lib.rs#L87-L167
fn to_yaml(py: Python, obj: &PyObject) -> Result<Value, PyConversionError> {
    macro_rules! return_cast {
        ($t:ty, $f:expr) => {
            if let Ok(val) = obj.downcast::<$t>(py) {
                return $f(val);
            }
        };
    }

    macro_rules! return_to_value {
        ($t:ty) => {
            if let Ok(val) = obj.extract::<$t>(py) {
                return serde_yaml::to_value(val).map_err(|error| {
                    PyConversionError::InvalidConversion {
                        error: format!("{}", error),
                    }
                });
            }
        };
    }

    if obj.is_none(py) {
        return Ok(Value::Null);
    }

    return_to_value!(String);
    return_to_value!(bool);
    return_to_value!(u64);
    return_to_value!(i64);

    return_cast!(PyDict, |x: &PyDict| {
        let mut map = serde_yaml::Mapping::new();
        for (key_obj, value) in x.iter() {
            let key = if key_obj.is_none() {
                Ok("null".to_string())
            } else if let Ok(val) = key_obj.extract::<bool>() {
                Ok(if val {
                    "true".to_string()
                } else {
                    "false".to_string()
                })
            } else if let Ok(val) = key_obj.str() {
                Ok(val.to_string())
            } else {
                Err(PyConversionError::DictKeyNotSerializable {
                    typename: key_obj
                        .to_object(py)
                        .as_ref(py)
                        .get_type()
                        .name()
                        .map(|x| x.to_string())
                        .unwrap_or_else(|_| "unknown".to_string()),
                })
            };
            map.insert(Value::String(key?), to_yaml(py, &value.to_object(py))?);
        }
        Ok(Value::Mapping(map))
    });

    return_cast!(PyList, |x: &PyList| {
        let v = x
            .iter()
            .map(|x| to_yaml(py, &x.to_object(py)))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Value::Sequence(v))
    });

    return_cast!(PyTuple, |x: &PyTuple| {
        let v = x
            .iter()
            .map(|x| to_yaml(py, &x.to_object(py)))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Value::Sequence(v))
    });

    return_cast!(PyFloat, |x: &PyFloat| {
        Ok(Value::Number(serde_yaml::Number::from(x.value())))
    });

    // At this point we can't cast it, set up the error object
    Err(PyConversionError::InvalidCast {
        typename: obj
            .as_ref(py)
            .get_type()
            .name()
            .map(|x| x.to_string())
            .unwrap_or_else(|_| "unknown".to_string()),
    })
}

fn value_to_object(val: Value, py: Python<'_>) -> PyObject {
    match val {
        Value::Null => py.None(),
        Value::Bool(x) => x.to_object(py),
        Value::Number(x) => {
            let oi64 = x.as_i64().map(|i| i.to_object(py));
            let ou64 = x.as_u64().map(|i| i.to_object(py));
            let of64 = x.as_f64().map(|i| i.to_object(py));
            oi64.or(ou64).or(of64).expect("number too large")
        }
        Value::String(x) => x.to_object(py),
        Value::Sequence(x) => {
            let inner: Vec<_> = x.into_iter().map(|x| value_to_object(x, py)).collect();
            inner.to_object(py)
        }
        Value::Mapping(x) => {
            let iter = x
                .into_iter()
                .map(|(k, v)| (value_to_object(k, py), value_to_object(v, py)));
            IntoPyDict::into_py_dict(iter, py).into()
        }
        Value::Tagged(_) => panic!("tagged values are not supported"),
    }
}

#[pymethods]
impl ToolsWrapper {
    // list all tools
    #[pyo3(signature = ())]
    fn list(&self, py: Python<'_>) -> PyResult<PyObject> {
        let tools = self.toolbox.describe();
        let tools = tools
            .into_iter()
            .map(|(name, t)| (name, t.description))
            .collect::<HashMap<_, _>>();
        let tools = tools.to_object(py);
        Ok(tools)
    }

    // invoke a tool
    #[pyo3(signature = (tool_name, input))]
    fn invoke(
        &self,
        py: Python<'_>,
        tool_name: &str,
        input: Option<&PyDict>,
    ) -> PyResult<PyObject> {
        // convert PyDict to a serde_yaml::Value
        let input = if let Some(input) = input {
            let input: PyObject = input.into();

            to_yaml(py, &input).map_err(|e| {
                pyo3::exceptions::PyException::new_err(format!("Invalid input: {}", e))
            })?
        } else {
            Value::default()
        };

        println!("invoking tool {} with input {:?}", tool_name, input);

        let output =
            invoke_simple_from_toolbox(self.toolbox.clone(), tool_name, input).map_err(|e| {
                pyo3::exceptions::PyException::new_err(format!("Tool invocation failed: {}", e))
            })?;

        let output = value_to_object(output, py);

        Ok(output)
    }
}

impl PythonTool {
    fn invoke_typed(
        &self,
        toolbox: Option<Rc<Toolbox>>,
        input: &PythonToolInput,
    ) -> Result<PythonToolOutput, ToolUseError> {
        let mut code = input.code.clone();

        let re = regex::Regex::new(r"open|exec|eval").unwrap();
        if re.is_match(&code) {
            return Err(ToolUseError::ToolInvocationFailed(
                "Python code contains forbidden keywords such as open|exec|eval".to_string(),
            ));
        }

        let tools = toolbox.map(ToolsWrapper::new);

        // dynamically add functions to a `tools` module
        if let Some(tools) = &tools {
            let mut tool_class_code = String::new();

            tool_class_code.push_str("class Tools:\n");

            tool_class_code.push_str("    def __init__(self, toolbox):\n");
            tool_class_code.push_str("        self.toolbox = toolbox\n");

            for (name, description) in tools.toolbox.as_ref().describe() {
                let inputs_parts = description.input_format.parts;
                let inputs = inputs_parts
                    .iter()
                    .map(|f| f.key.clone())
                    .collect::<Vec<_>>()
                    .join(", ");
                let inputs = if inputs.is_empty() {
                    "".to_string()
                } else {
                    format!("(self, {})", inputs)
                };

                let dict = inputs_parts
                    .iter()
                    .map(|f| {
                        let name = &f.key;
                        format!("\"{}\": {}", name, name)
                    })
                    .collect::<Vec<_>>()
                    .join(", ");

                // in snake case
                tool_class_code.push_str(&format!(
                    "    def {}{}:\n        return self.toolbox.invoke(\"{}\", {{{}}})\n",
                    name.to_case(Case::Snake),
                    inputs,
                    name,
                    dict
                ));

                // in Pascal case
                tool_class_code.push_str(&format!(
                    "    def {}{}:\n        return self.toolbox.invoke(\"{}\", {{{}}})\n",
                    name.to_case(Case::Pascal),
                    inputs,
                    name,
                    dict
                ));
            }

            // add list function
            tool_class_code.push_str("    def list(self):\n");
            tool_class_code.push_str("        return self.toolbox.list()\n");

            tool_class_code.push_str("tools = Tools(toolbox)\n");

            // prepend the tool class code to the user code
            code = format!("{}\n{}", tool_class_code, code);
        }

        // print!("{}", code);

        let res: PyResult<(String, String)> = Python::with_gil(|py| {
            // println!("Python version: {}", py.version());

            let globals = if let Some(tools) = tools {
                let tools_cell = PyCell::new(py, tools)?;
                [("toolbox", tools_cell)].into_py_dict(py)
            } else {
                PyDict::new(py)
            };

            // capture stdout and stderr
            let sys = py.import("sys")?;

            let stdout = Logging::default();
            let py_stdout_cell = PyCell::new(py, stdout)?;
            let py_stdout = py_stdout_cell.borrow_mut();
            sys.setattr("stdout", py_stdout.into_py(py))?;

            let stderr = Logging::default();
            let py_stderr_cell = PyCell::new(py, stderr)?;
            let py_stderr = py_stderr_cell.borrow_mut();
            sys.setattr("stderr", py_stderr.into_py(py))?;

            // FUTURE(ssoudan) pass something in

            // run code
            Python::run(py, &code, globals.into(), None)?;

            // NOFUTURE(ssoudan) get something out

            let stdout = py_stdout_cell.borrow().output.clone();
            let stderr = py_stderr_cell.borrow().output.clone();

            Ok((stdout, stderr))
        });

        let (stdout, stderr) = res.map_err(|e| {
            ToolUseError::ToolInvocationFailed(format!("Python code execution failed: {}", e))
        })?;

        Ok(PythonToolOutput { stdout, stderr })
    }
}

impl Tool for PythonTool {
    fn description(&self) -> ToolDescription {
        ToolDescription::new(
            "SandboxedPython",
            "A tool that executes sandboxed Python code. Only stdout and stderr are captured and made available. ",
            r#"Use this to transform data. To use other Tools from here: `input = {...}; output = tools.tool_name(**input); print(output["field_xxx"])`. The `output` is a object. open|exec|eval are forbidden."#,
            PythonToolInput::describe(),
            PythonToolOutput::describe(),
        )
    }

    fn invoke(&self, input: serde_yaml::Value) -> Result<serde_yaml::Value, ToolUseError> {
        let input = serde_yaml::from_value(input)?;
        let output = self.invoke_typed(None, &input)?;
        Ok(serde_yaml::to_value(output)?)
    }
}

impl AdvancedTool for PythonTool {
    fn invoke_with_toolbox(
        &self,
        toolbox: Rc<Toolbox>,
        input: Value,
    ) -> Result<Value, ToolUseError> {
        let input = serde_yaml::from_value(input)?;
        let output = self.invoke_typed(Some(toolbox), &input)?;
        Ok(serde_yaml::to_value(output)?)
    }
}

#[cfg(test)]
mod tests {
    use pyo3::indoc::indoc;
    use pyo3::types::PyDict;

    use super::*;
    use crate::tools::dummy::DummyTool;

    #[test]
    fn test_python_tool() {
        let tool = PythonTool::default();
        let input = PythonToolInput {
            code: indoc! {
            r#"print('hello')
               t = toolbox.list()
               print("tools=", t)
               
               d = tools.dummy(blah="ahah")
               print("dummy=", d)
               
               "#}
            .to_string(),
        };
        let mut toolbox = Toolbox::default();
        toolbox.add_tool(DummyTool::default());
        let toolbox = Rc::new(toolbox);

        let output = tool.invoke_typed(Some(toolbox), &input).unwrap();
        assert_eq!(
            output.stdout,
            "hello\ntools= {'Dummy': 'A tool to test stuffs.'}\ndummy= {'something': 'ahah and something else'}\n"
        );
        assert_eq!(output.stderr, "");
    }

    #[pyfunction]
    fn add_one(x: i64) -> i64 {
        x + 1
    }

    #[pymodule]
    fn foo(_py: Python<'_>, foo_module: &PyModule) -> PyResult<()> {
        foo_module.add_function(wrap_pyfunction!(add_one, foo_module)?)?;
        Ok(())
    }

    #[test]
    fn test_run_with_pyo3() {
        pyo3::append_to_inittab!(foo);
        Python::with_gil(|py| {
            let locals = PyDict::new(py);

            // capture stdout
            let sys = py.import("sys")?;
            let stdout = Logging::default();
            let py_stdout_cell = PyCell::new(py, stdout).unwrap();
            let stderr = Logging::default();
            let py_stderr_cell = PyCell::new(py, stderr).unwrap();

            let py_stdout = py_stdout_cell.borrow_mut();
            let py_stderr = py_stderr_cell.borrow_mut();
            sys.setattr("stdout", py_stdout.into_py(py))?;
            sys.setattr("stderr", py_stderr.into_py(py))?;

            let res = Python::run(
                py,
                indoc! {
                r#"import foo;
                   a = 12
                   b = foo.add_one(a)
                   print("b=", b)                                                                                                                
                  "#},
                None,
                locals.into(),
            );

            assert_eq!(locals.get_item("a").unwrap().extract::<i64>().unwrap(), 12);
            assert_eq!(locals.get_item("b").unwrap().extract::<i64>().unwrap(), 13);

            let stdout = py_stdout_cell.borrow();
            assert_eq!(stdout.output, "b= 13\n");

            let stderr = py_stderr_cell.borrow();
            assert_eq!(stderr.output, "");

            res
        }).unwrap();
    }
}
