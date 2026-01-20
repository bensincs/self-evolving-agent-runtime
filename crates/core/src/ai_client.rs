// crates/core/src/ai_client.rs

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Abstract AI/LLM client for chat completions with tool support.
///
/// Implementations can use Azure Foundry, OpenAI, Ollama, etc.
pub trait AiClient {
    /// Send a chat completion request with optional tools.
    fn chat(&self, request: ChatRequest) -> Result<ChatResponse>;
}

/// A chat completion request.
#[derive(Debug, Clone, Serialize)]
pub struct ChatRequest {
    pub messages: Vec<Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<String>,
}

impl ChatRequest {
    pub fn new(messages: Vec<Value>) -> Self {
        Self {
            messages,
            tools: Vec::new(),
            tool_choice: None,
        }
    }

    pub fn with_tools(mut self, tools: Vec<Value>) -> Self {
        self.tools = tools;
        self.tool_choice = Some("auto".to_string());
        self
    }
}

/// A chat completion response.
#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
pub struct ChatChoice {
    pub message: ChatMessage,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<ChatToolCall>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChatToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: ChatToolFunction,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChatToolFunction {
    pub name: String,
    /// Raw JSON string of the arguments.
    pub arguments: String,
}
