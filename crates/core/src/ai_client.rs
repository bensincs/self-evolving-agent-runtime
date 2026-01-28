// crates/core/src/ai_client.rs

//! Simple AI client for the Responses API.

use anyhow::Result;
use serde::Serialize;

/// Response item from the Responses API.
#[derive(Debug, Clone)]
pub enum ResponseItem {
    /// Text message from the model.
    Message(String),
    /// Tool/function call.
    FunctionCall {
        call_id: String,
        name: String,
        arguments: String,
    },
}

/// A response from the Responses API.
#[derive(Debug, Clone)]
pub struct Response {
    pub items: Vec<ResponseItem>,
}

impl Response {
    /// Get the text content if this is a message response.
    pub fn text(&self) -> Option<&str> {
        for item in &self.items {
            if let ResponseItem::Message(text) = item {
                return Some(text);
            }
        }
        None
    }

    /// Get function calls if any.
    pub fn function_calls(&self) -> Vec<&ResponseItem> {
        self.items
            .iter()
            .filter(|item| matches!(item, ResponseItem::FunctionCall { .. }))
            .collect()
    }

    /// Check if this response has function calls.
    pub fn has_function_calls(&self) -> bool {
        self.items
            .iter()
            .any(|item| matches!(item, ResponseItem::FunctionCall { .. }))
    }
}

/// Input item for the Responses API.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum InputItem {
    #[serde(rename = "message")]
    Message {
        role: String,
        content: Vec<ContentPart>,
    },
    #[serde(rename = "function_call")]
    FunctionCall {
        call_id: String,
        name: String,
        arguments: String,
    },
    #[serde(rename = "function_call_output")]
    FunctionCallOutput { call_id: String, output: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ContentPart {
    #[serde(rename = "input_text")]
    InputText { text: String },
    #[serde(rename = "output_text")]
    OutputText { text: String },
}

impl InputItem {
    pub fn user(text: impl Into<String>) -> Self {
        Self::Message {
            role: "user".to_string(),
            content: vec![ContentPart::InputText { text: text.into() }],
        }
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        Self::Message {
            role: "assistant".to_string(),
            content: vec![ContentPart::OutputText { text: text.into() }],
        }
    }

    pub fn function_call(
        call_id: impl Into<String>,
        name: impl Into<String>,
        arguments: impl Into<String>,
    ) -> Self {
        Self::FunctionCall {
            call_id: call_id.into(),
            name: name.into(),
            arguments: arguments.into(),
        }
    }

    pub fn function_output(call_id: impl Into<String>, output: impl Into<String>) -> Self {
        Self::FunctionCallOutput {
            call_id: call_id.into(),
            output: output.into(),
        }
    }
}

/// Tool definition for the Responses API.
#[derive(Debug, Clone, Serialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

impl Tool {
    pub fn function(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        Self {
            tool_type: "function".to_string(),
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }
}

/// AI client trait for the Responses API.
pub trait AiClient {
    fn respond(
        &self,
        instructions: &str,
        input: Vec<InputItem>,
        tools: &[Tool],
    ) -> Result<Response>;
}
