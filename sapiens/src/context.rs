//! Maintain the context for the bot.
use std::fmt::{Debug, Formatter};

use tiktoken_rs::async_openai::num_tokens_from_messages;
use tiktoken_rs::model::get_context_size;

use crate::openai::{ChatCompletionRequestMessage, Role};

/// A trait for formatting entries for the chat history
pub trait ChatEntryFormatter {
    /// Format the entry
    fn format(&self, entry: &ChatEntry) -> String;
}

/// An error that can occur when adding a prompt to the chat history
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// The prompt is too long
    #[error("The prompt is too long")]
    PromptTooLong,
}

/// A history entry
#[derive(Debug, Clone)]
pub struct ChatEntry {
    /// The role
    pub role: Role,
    /// The message
    pub msg: String,
}

impl From<&ChatCompletionRequestMessage> for ChatEntry {
    fn from(msg: &ChatCompletionRequestMessage) -> Self {
        Self {
            role: msg.role.clone(),
            msg: msg.content.clone(),
        }
    }
}

/// Maintain a chat history that can be truncated (from the head) to ensure
/// we have enough tokens to complete the task
///
/// The prompt is the part of the history that we want to stay at the top of the
/// history. The chitchat is the rest of the history.
///
/// Add the prompting messages to the history with [ChatHistory::add_prompts].
///
/// To ensure we have enough tokens to complete the task, we truncate the
/// chitchat history when new messages are added - with
/// [ChatHistory::add_chitchat].
#[derive(Clone)]
pub struct ChatHistory {
    /// The model
    model: String,
    /// The maximum number of tokens we can have in the history for the model
    max_token: usize,
    /// The minimum number of tokens we need to complete the task
    min_token_for_completion: usize,
    /// The 'prompt' (aka messages we want to stay at the top of the history)
    prompt: Vec<ChatCompletionRequestMessage>,
    /// Num token for the prompt
    prompt_num_tokens: usize,
    /// The other messages
    chitchat: Vec<ChatCompletionRequestMessage>,
}

impl Debug for ChatHistory {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChatHistory")
            .field("model", &self.model)
            .field("max_token", &self.max_token)
            .field("min_token_for_completion", &self.min_token_for_completion)
            .field("prompt_num_tokens", &self.prompt_num_tokens)
            .finish()
    }
}

impl ChatHistory {
    /// Create a new chat history
    pub fn new(model: String, min_token_for_completion: usize) -> Self {
        let max_token = get_context_size(&model);
        Self {
            max_token,
            model,
            min_token_for_completion,
            prompt: vec![],
            prompt_num_tokens: 0,
            chitchat: vec![],
        }
    }

    /// add a prompt to the history
    pub fn add_prompts(&mut self, prompts: &[(Role, String)]) {
        for (role, content) in prompts {
            let msg = ChatCompletionRequestMessage {
                role: role.clone(),
                content: content.clone(),
                name: None,
            };
            self.prompt.push(msg);
        }

        // update the prompt_num_tokens
        self.prompt_num_tokens = num_tokens_from_messages(&self.model, &self.prompt).unwrap();
    }

    /// add a message to the chitchat history, and prune the history if needed
    /// returns the number of messages in the chitchat history
    pub fn add_chitchat(&mut self, entry: ChatEntry) -> Result<usize, Error> {
        let msg = ChatCompletionRequestMessage {
            role: entry.role,
            content: entry.msg,
            name: None,
        };

        self.chitchat.push(msg);

        // prune the history if needed
        self.purge()
    }

    /// uses [tiktoken_rs::num_tokens_from_messages] prune
    /// the chitchat history starting from the head until we have enough
    /// tokens to complete the task
    pub fn purge(&mut self) -> Result<usize, Error> {
        // FIXME(ssoudan) preserve the alternance of roles

        let token_budget = self.max_token.saturating_sub(self.prompt_num_tokens);

        if token_budget == 0 {
            // we can't even fit the prompt
            self.chitchat = vec![];
            return Err(Error::PromptTooLong);
        }

        // loop until we have enough available tokens to complete the task
        while self.chitchat.len() > 1 {
            let num_tokens = num_tokens_from_messages(&self.model, &self.chitchat).unwrap();
            if num_tokens <= token_budget - self.min_token_for_completion {
                return Ok(self.chitchat.len());
            }
            self.chitchat.remove(0);
        }

        Ok(self.chitchat.len())
    }

    /// iterate over the prompt and chitchat messages
    pub fn iter(&self) -> impl Iterator<Item = &ChatCompletionRequestMessage> {
        self.prompt.iter().chain(self.chitchat.iter())
    }

    /// format the history using the given formatter
    pub fn format<T>(&self, formatter: &T) -> Vec<String>
    where
        T: ChatEntryFormatter + ?Sized,
    {
        self.iter()
            .map(|msg| {
                let e = ChatEntry {
                    role: msg.role.clone(),
                    msg: msg.content.clone(),
                };
                formatter.format(&e)
            })
            .collect::<Vec<_>>()
    }
}

impl From<&ChatHistory> for Vec<ChatCompletionRequestMessage> {
    fn from(val: &ChatHistory) -> Self {
        val.iter().cloned().collect()
    }
}
