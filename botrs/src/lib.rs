//! Botrs library

/// Tools
pub mod tools;

pub(crate) mod context;

use std::rc::Rc;

use async_openai::types::{ChatCompletionRequestMessage, CreateChatCompletionRequest, Role};
use colored::Colorize;
use context::ChatHistory;
use llm_chain::parsing::find_yaml;
use llm_chain::tools::{ToolDescription, ToolUseError};
use serde::{Deserialize, Serialize};

use crate::tools::{invoke_from_toolbox, Toolbox};

fn create_system_prompt() -> String {
    "You are an automated agent named botGPT interacting with the WORLD".to_string()
}

const PREFIX: &str = r"You are botGPT, large language model assisting the WORLD. Use available tools to answer the question as best as you can.
You will proceed in a OODA loop made of the following steps:
- Observations: What do you know? What is your source? What don't you know? You might want to note down important information for later like relevant past Action results. 
- Orientation: Plan the intermediate objectives along the path to answer the original question. Make a list of current objectives. 
- Decision: Choose what to do first to answer the question. Why? What are the pros and cons of this decision?
- Action: Take a single Action consisting of exactly one tool invocation. The available Tools listed below. Use Conclude Tool when you have the final answer to the original question.

# Notes: 
- Use Conclude Tool with your conclusion (as text) once you have the final answer to the original question.
- Template expansion is not supported in Action. If you need to pass information from on action to another, you have to pass them manually.
- There are no APIs available to you. You have to use the Tools.
";

const TOOL_PREFIX: &str = r"
# The following are the ONLY Tools you can use for your Actions:
";

const FORMAT: &str = r"
# Format of your response

Please use the following format for your response - no need to be verbose:
====================
## Observations:
- ...
## Orientation:
- ...
## Decision:
- ...
## The ONLY Action: <Do not give multiple command. Only one per response.>
```yaml
command: <ToolName>
input:
  <... using the `input_format` for the Tool ...>
```
====================
";

const PROTO_EXCHANGE_1: &str = r"
## Original question: Sort in ascending order: [2, 3, 1, 4, 5]. 
";

const PROTO_EXCHANGE_2: &str = r#"
## Observations:
- The given list to sort is [2, 3, 1, 4, 5].
- I need to sort this list in ascending order.
## Orientation:
- SandboxedPython can be used to sort the list.
## Decision:
- We can use the sorted() function of Python to sort the list.
## The ONLY Action:
```yaml
command: SandboxedPython
input:
  code: |
    lst = [2, 3, 1, 4, 5]
    sorted_list = sorted(lst)
    print(f"The sorted list is {sorted_list}")
```
"#;

const PROTO_EXCHANGE_3: &str = r"
# Action result:
```yaml
status: 0
stdout: |
  The sorted list is [1, 2, 3, 4, 5]
stderr: ''
```
# Your turn
Original question: Sort the following list: [2, 3, 1, 4, 5]
Observations, Orientation, Decision, The ONLY Action?
";

const PROTO_EXCHANGE_4: &str = r"
## Observations:
- We needed to sort the list in ascending order.
- We have the result of the Action.
- We have the sorted list: [1, 2, 3, 4, 5].
## Orientation:
- I know the answer to the original question.
## Decision:
- Use the Conclude Tool to terminate the task with the sorted list.
## The ONLY Action:
```yaml
command: Conclude
input:
  original_question: |
    Sort in ascending order: [2, 3, 1, 4, 5]
  conclusion: |
    The ascending sorted list is [1, 2, 3, 4, 5].
```
";

fn create_tool_description(tb: &Toolbox) -> String {
    let prefix = TOOL_PREFIX.to_string();

    let tool_desc = tb.describe();

    let tool_desc: Vec<ToolDescription> = tool_desc.into_values().collect();

    // yaml serialize the tool description
    let tool_desc = serde_yaml::to_string(&tool_desc).unwrap();

    prefix + &tool_desc
}

fn create_tool_warm_up(tb: &Toolbox) -> String {
    let prefix = PREFIX.to_string();
    let tool_prompt = create_tool_description(tb);
    prefix + FORMAT + &tool_prompt
}

fn build_task_prompt(task: &str) -> String {
    format!(
        "# Your turn\nOriginal question: {}\nObservations, Orientation, Decision, The ONLY Action?",
        task
    )
}

/// Run a task with a set of tools
pub async fn something_with_rooms(toolbox: Toolbox, task: &str, max_steps: usize, model: String) {
    let warm_up_prompt = create_tool_warm_up(&toolbox);
    let system_prompt = create_system_prompt();

    // build the warm-up exchange with the user
    let prompt = [
        (Role::System, system_prompt),
        (Role::User, warm_up_prompt),
        (Role::Assistant, "Understood.".to_string()),
        (Role::User, PROTO_EXCHANGE_1.to_string()),
        (Role::Assistant, PROTO_EXCHANGE_2.to_string()),
        (Role::User, PROTO_EXCHANGE_3.to_string()),
        (Role::Assistant, PROTO_EXCHANGE_4.to_string()),
    ];

    let mut chat_history = ChatHistory::new(model.clone(), 256);

    chat_history.add_prompts(&prompt);

    // Now we are ready to start the task
    let task_prompt = build_task_prompt(task);

    chat_history
        .add_chitchat(Role::User, task_prompt.to_string())
        .expect("The task prompt is too long for the model");

    // Let's print the chat history so far - yellow for the system, green for the
    // user, blue for the assistant
    for message in chat_history.iter() {
        match message.role {
            Role::System => println!("{}", message.content.yellow()),
            Role::User => println!("{}", message.content.green()),
            Role::Assistant => println!("{}", message.content.blue()),
        }
        println!("=============")
    }

    // Build a tool description to inject it into the chat on error
    // let tool_desc = create_tool_description(&toolbox);

    let toolbox = Rc::new(toolbox);

    let openai_client = async_openai::Client::new();

    for _ in 1..max_steps {
        let messages: Vec<ChatCompletionRequestMessage> = (&chat_history).into();
        let input = CreateChatCompletionRequest {
            model: model.clone(),
            messages,
            temperature: None,
            top_p: None,
            n: Some(1),
            stream: None,
            stop: None,
            max_tokens: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            user: None,
        };
        let res = openai_client.chat().create(input).await.unwrap();
        // dbg!(&res);

        let message_text = res.choices.first().unwrap().message.content.clone();

        println!("{}", message_text.blue());

        let l = chat_history
            .add_chitchat(Role::Assistant, message_text.clone())
            .expect("The assistant response is too long for the model");
        println!(
            "============= {:>3} messages in the chat history =============",
            l
        );

        let resp = invoke_tool(toolbox.clone(), &message_text);
        let l = match resp {
            Ok(x) => {
                // check if the task is done
                let termination_messages = toolbox.termination_messages();
                if !termination_messages.is_empty() {
                    for message in termination_messages {
                        println!(
                            "The original question was: {} ",
                            message.original_question.green()
                        );
                        println!("And the conclusion is: {} ", message.conclusion.blue());
                    }

                    break;
                }

                let content = format!("# Action result: \n```yaml\n{}```\n{}", x, task_prompt);

                println!("{}", content.green());

                chat_history
                    .add_chitchat(Role::User, content.clone())
                    .expect("The user response is too long for the model")
            }
            Err(e) => {
                let content = format!(
                    "# Failed with:\n{:?}\nWhat was incorrect in previous response?\n{}",
                    e, task_prompt
                );
                println!("{}", content.red());

                // check if the task is done
                let termination_messages = toolbox.termination_messages();
                if !termination_messages.is_empty() {
                    for message in termination_messages {
                        println!(
                            "The original question was: {} ",
                            message.original_question.green()
                        );
                        println!("And the conclusion is: {} ", message.conclusion.blue());
                    }

                    break;
                }

                chat_history
                    .add_chitchat(Role::User, content.clone())
                    .expect("The user response is too long for the model")
            }
        };
        println!(
            "============= {:>3} messages in the chat history =============",
            l
        );
    }
}

/// Try to find the tool invocation from the chat message and invoke the
/// corresponding tool.
///
/// If multiple tool invocations are found, only the first one is used.
pub fn invoke_tool(tools: Rc<Toolbox>, data: &str) -> Result<String, ToolUseError> {
    let tool_invocations: Vec<ToolInvocationInput> = find_yaml::<ToolInvocationInput>(data)?;
    if tool_invocations.is_empty() {
        return Err(ToolUseError::ToolInvocationFailed(
            "No Action found".to_string(),
        ));
    }

    // Take the first invocation - the list is reversed
    let invocation_input = &tool_invocations.last().unwrap();
    let input = invocation_input.input.clone();
    let output = invoke_from_toolbox(tools, &invocation_input.command, input)?;
    Ok(serde_yaml::to_string(&output).unwrap())
}

#[derive(Serialize, Deserialize, Debug)]
struct ToolInvocationInput {
    command: String,
    input: serde_yaml::Value,
}

#[cfg(test)]
mod tests {
    use indoc::indoc;

    use super::*;
    use crate::tools::dummy::DummyTool;
    use crate::tools::python::PythonTool;

    #[test]
    fn test_tool_invocation() {
        let data = indoc! {r#"
        # Action
        ```yaml        
        command: SandboxedPython
        input:
            code: |
                print("Hello world!")          
        ```
        "#};

        let mut toolbox = Toolbox::default();
        toolbox.add_advanced_tool(PythonTool::default());

        let toolbox = Rc::new(toolbox);

        let output = invoke_tool(toolbox, data).unwrap();
        assert_eq!(output, "stdout: |\n  Hello world!\nstderr: ''\n");
    }

    #[test]
    fn test_tool_invocation_in_python() {
        let data = indoc! {r#"
        # Action
        ```yaml        
        command: SandboxedPython
        input:
            code: |
                print("Hello world!")
                rooms = toolbox.invoke("Dummy", {"blah": "blah"})
                print(rooms)
                rooms = tools.dummy(blah="blah")
                print(rooms)          
        ```
        "#};

        let mut toolbox = Toolbox::default();
        toolbox.add_advanced_tool(PythonTool::default());
        toolbox.add_tool(DummyTool::default());

        let toolbox = Rc::new(toolbox);

        let output = invoke_tool(toolbox, data).unwrap();
        assert_eq!(
            output,
            "stdout: |\n  Hello world!\n  {'something': 'blah and something else'}\n  {'something': 'blah and something else'}\nstderr: ''\n"
        );
    }

    #[test]
    fn test_multiple_tool_invocations() {
        let data = indoc! {r#"
        # Action
        ```yaml        
        command: SandboxedPython
        input:
            code: |
                print("Hello world 1!")          
        ```
        
        # And another action
        ```yaml        
        command: SandboxedPython
        input:
            code: |
                print("Hello world 2!")          
        ```
        
        # And yet another action
        ```        
        command: SandboxedPython
        input:
            code: |
                print("Hello world 3!")          
        ```
        "#};

        let mut toolbox = Toolbox::default();
        toolbox.add_advanced_tool(PythonTool::default());

        let toolbox = Rc::new(toolbox);

        let output = invoke_tool(toolbox, data).unwrap();
        assert_eq!(output, "stdout: |\n  Hello world 1!\nstderr: ''\n");
    }
}
