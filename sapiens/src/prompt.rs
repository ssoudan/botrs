use std::fmt;

use crate::context::ChatHistory;
use crate::openai::Role;
use crate::tools::{ToolDescription, ToolUseError, Toolbox};

const PREFIX: &str = r"You are Sapiens, a large language model assisting the WORLD. Use available tools to answer the question as best as you can.
You will proceed iteratively using an OODA loop.

Action result will be provided to you. The loop will repeated until you have the answer to the original question. No task is complete until the Conclude Tool is used to provide the answer.
You cannot use jinja2 templating in your response. Be concise. 
";

const TOOL_PREFIX: &str = r"
# The following are the ONLY Tools you can use for your Actions:
";

const FORMAT: &str = r"
# Format of your response

Please use the following format for your response - no need to be verbose. Comments are in bold and should be removed from your response.
====================
## Observations: 
**What do you know to be true? What do you you don't know? What are your sources? Note down important information for later.**
- ...
## Orientation: 
**Plan the intermediate objectives to answer the original question. Maintain a list of current objectives updated as you go.**
- ...
## Decision: 
**Decide what to do first to answer the question. Why? How will you if it succeeds? How will you if it fails?**
- ...
## The ONLY Action: 
**Take a single Action consisting of exactly one tool invocation (`command` and `input`). The available Tools listed below. Use Conclude Tool when you have the final answer to the original question. Never give more than one `command` and one `input` fields. Never give more than one YAML.**  
```yaml
command: <ToolName>
input:
  <... using the `input_format` for the Tool ...>
```
====================
";

const PROTO_EXCHANGE_2: &str = r#"
## Observations:
- The given list to sort is [2, 3, 1, 4, 5].
- I need to sort this list in ascending order.
## Orientation:
- SandboxedPython can be used to sort the list.
- I need to use the Conclude Tool to terminate the task when I have the sorted list
- I need to provide the conclusion in plain text to the Conclude Tool.
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
# Action SandboxedPython result:
```yaml
stdout: |
  The sorted list is [1, 2, 3, 4, 5]
stderr: ''
```
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

/// Prompt manager
#[derive(Clone)]
pub(crate) struct Manager {
    toolbox: Toolbox,
}

impl Manager {
    /// Create a new prompt manager
    pub fn new(toolbox: Toolbox) -> Self {
        Self { toolbox }
    }

    /// Create the prompt describing the tools
    async fn create_tool_description(&self) -> String {
        let prefix = TOOL_PREFIX.to_string();

        let tool_desc = self.toolbox.describe().await;

        let mut tool_desc: Vec<ToolDescription> = tool_desc.into_values().collect();

        // sort by tool name
        tool_desc.sort_by(|a, b| a.name.cmp(&b.name));

        // yaml serialize the tool description
        let tool_desc = serde_yaml::to_string(&tool_desc).unwrap();

        prefix + &tool_desc
    }

    /// Create the prompt describing the tools and how to use them
    async fn create_tool_warm_up(&self) -> String {
        let prefix = PREFIX.to_string();
        let tool_prompt = self.create_tool_description().await;
        prefix + FORMAT + &tool_prompt
    }

    /// Create the prompt for the task
    pub(crate) fn build_task_prompt(&self, task: &str) -> Task {
        Task {
            task: task.to_string(),
        }
    }

    /// Create the 'system' prompt to describe the roles.
    fn create_system_prompt(&self) -> String {
        "You are an automated agent named Sapiens interacting with the WORLD. Listen to the WORLD!"
            .to_string()
    }

    pub(crate) async fn populate_chat_history(&self, chat_history: &mut ChatHistory) {
        let warm_up_prompt = self.create_tool_warm_up().await;
        let system_prompt = self.create_system_prompt();

        let warmup_task = Task {
            task: "Sort in ascending order: [2, 3, 1, 4, 5]".to_string(),
        };

        let prompt = [
            (Role::System, system_prompt.trim().to_string()),
            (Role::User, warm_up_prompt.trim().to_string()),
            (Role::Assistant, "Understood.".to_string()),
            (Role::User, warmup_task.to_prompt()),
            (Role::Assistant, PROTO_EXCHANGE_2.trim().to_string()),
            (
                Role::User,
                (format!("{}{}", PROTO_EXCHANGE_3, warmup_task.to_prompt()))
                    .trim()
                    .to_string(),
            ),
            (Role::Assistant, PROTO_EXCHANGE_4.trim().to_string()),
        ];

        chat_history.add_prompts(&prompt);
    }
}

/// Task-related prompts
///
/// Use [`Display`] to get the prompt.
#[derive(Debug)]
pub struct Task {
    task: String,
}

impl Task {
    /// Create the prompt for the task
    fn to_prompt(&self) -> String {
        // NOTE(ssoudan) what about bringing focus on the answer before the tool result?
        format!(
            "# Your turn\nOriginal question: {}\nDo you have the answer? Use the Conclude Tool to terminate the task.\nObservations, Orientation, Decision, The ONLY Action?",
            self.task
        )
    }

    /// Create the prompt to react to an action failure
    pub(crate) fn action_failed_prompt(
        &self,
        tool_name: impl AsRef<str>,
        e: &ToolUseError,
    ) -> String {
        format!(
            "# Action {} failed with:\n{:?}\nWhat was incorrect in previous response?\n{}",
            tool_name.as_ref(),
            e,
            self.to_prompt()
        )
    }

    /// Create the prompt to react to an action success
    pub(crate) fn action_success_prompt(
        &self,
        tool_name: impl AsRef<str>,
        result: impl AsRef<str>,
    ) -> String {
        format!(
            "# Action {} result: \n```yaml\n{}```\n{}",
            tool_name.as_ref(),
            result.as_ref(),
            &self
        )
    }
}

impl fmt::Display for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_prompt())
    }
}
