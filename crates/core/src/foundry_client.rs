// crates/core/src/foundry_client.rs

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::ai_client::{
    AiClient, ChatMessage, ChatRequest, ChatResponse, ChatToolCall, ChatToolFunction,
};

/// API mode for the client.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ApiMode {
    /// Standard /chat/completions endpoint
    ChatCompletions,
    /// Modern /responses endpoint (for Codex, gpt-5, etc.)
    Responses,
}

/// Chat client for Microsoft AI Foundry (Azure OpenAI).
///
/// Expects the following environment variables:
///
/// - FOUNDRY_ENDPOINT
///     e.g. "https://myresource.openai.azure.com"
///
/// - FOUNDRY_CHAT_DEPLOYMENT
///     e.g. "gpt-4o"
///
/// - FOUNDRY_API_KEY
///     your Azure OpenAI / Foundry API key
///
/// - FOUNDRY_API_VERSION (optional)
///     default: "2024-02-15-preview"
pub struct FoundryClient {
    client: Client,
    url: String,
    api_key: String,
    mode: ApiMode,
    model: String, // deployment/model name for Responses API
}

impl FoundryClient {
    /// Construct with explicit parameters (defaults to ChatCompletions mode).
    pub fn new(endpoint: &str, deployment: &str, api_key: &str, api_version: Option<&str>) -> Self {
        Self::new_with_mode(
            endpoint,
            deployment,
            api_key,
            api_version,
            ApiMode::ChatCompletions,
        )
    }

    /// Construct with explicit parameters and mode selection.
    pub fn new_with_mode(
        endpoint: &str,
        deployment: &str,
        api_key: &str,
        api_version: Option<&str>,
        mode: ApiMode,
    ) -> Self {
        let api_version = api_version.unwrap_or("2024-02-15-preview");
        let url = Self::build_url(endpoint, deployment, api_version, mode);

        Self {
            client: Client::new(),
            url,
            api_key: api_key.to_string(),
            mode,
            model: deployment.to_string(),
        }
    }

    /// Construct from environment variables using FOUNDRY_CHAT_DEPLOYMENT.
    pub fn from_env() -> Result<Self> {
        Self::from_env_with_deployment_var("FOUNDRY_CHAT_DEPLOYMENT")
    }

    /// Construct from environment variables with a custom deployment env var.
    ///
    /// This allows using different models by specifying different env vars,
    /// e.g. `FOUNDRY_MUTATION_DEPLOYMENT` for a coding-focused model.
    ///
    /// Auto-detects Codex/gpt-5 models and uses Responses API for them.
    /// Can override with FOUNDRY_API_MODE=responses or FOUNDRY_API_MODE=chat.
    pub fn from_env_with_deployment_var(deployment_var: &str) -> Result<Self> {
        let endpoint = std::env::var("FOUNDRY_ENDPOINT").context("FOUNDRY_ENDPOINT not set")?;

        let deployment =
            std::env::var(deployment_var).with_context(|| format!("{} not set", deployment_var))?;

        let api_key = std::env::var("FOUNDRY_API_KEY").context("FOUNDRY_API_KEY not set")?;

        let api_version = std::env::var("FOUNDRY_API_VERSION")
            .unwrap_or_else(|_| "2024-02-15-preview".to_string());

        // Auto-detect: use Responses API for Codex/gpt-5 models
        let deployment_lower = deployment.to_lowercase();
        let needs_responses = deployment_lower.contains("codex")
            || deployment_lower.starts_with("gpt-5")
            || deployment_lower.starts_with("o1")
            || deployment_lower.starts_with("o3");

        // Allow explicit override via env var
        let mode = match std::env::var("FOUNDRY_API_MODE").as_deref() {
            Ok("responses") => ApiMode::Responses,
            Ok("chat") => ApiMode::ChatCompletions,
            _ => {
                if needs_responses {
                    ApiMode::Responses
                } else {
                    ApiMode::ChatCompletions
                }
            }
        };

        if mode == ApiMode::Responses {
            eprintln!(
                "[FoundryClient] Using Responses API for deployment: {}",
                deployment
            );
            eprintln!(
                "[FoundryClient] URL: {}",
                Self::build_url(&endpoint, &deployment, &api_version, mode)
            );
        }

        Ok(Self::new_with_mode(
            &endpoint,
            &deployment,
            &api_key,
            Some(&api_version),
            mode,
        ))
    }

    fn build_url(endpoint: &str, deployment: &str, api_version: &str, mode: ApiMode) -> String {
        let (path, version) = match mode {
            ApiMode::ChatCompletions => (
                format!("openai/deployments/{}/chat/completions", deployment),
                api_version.to_string(),
            ),
            // Responses API requires newer API version
            ApiMode::Responses => (
                "openai/responses".to_string(),
                "2025-03-01-preview".to_string(),
            ),
        };
        format!(
            "{}/{}?api-version={}",
            endpoint.trim_end_matches('/'),
            path,
            version,
        )
    }

    /// Convert chat messages to input items for Responses API.
    fn messages_to_input(
        messages: &[serde_json::Value],
    ) -> (Option<String>, Vec<serde_json::Value>) {
        let mut instructions = None;
        let mut input_items = Vec::new();

        for msg in messages {
            let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("user");
            let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");

            match role {
                "system" => {
                    // System message becomes instructions
                    instructions = Some(content.to_string());
                }
                "user" => {
                    input_items.push(serde_json::json!({
                        "type": "message",
                        "role": "user",
                        "content": [{
                            "type": "input_text",
                            "text": content
                        }]
                    }));
                }
                "assistant" => {
                    // Check for tool calls
                    if let Some(tool_calls) = msg.get("tool_calls").and_then(|v| v.as_array()) {
                        for tc in tool_calls {
                            if let (Some(id), Some(func)) = (tc.get("id"), tc.get("function")) {
                                let name = func.get("name").and_then(|v| v.as_str()).unwrap_or("");
                                let args = func
                                    .get("arguments")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("{}");
                                input_items.push(serde_json::json!({
                                    "type": "function_call",
                                    "id": id,
                                    "call_id": id,
                                    "name": name,
                                    "arguments": args
                                }));
                            }
                        }
                    } else {
                        input_items.push(serde_json::json!({
                            "type": "message",
                            "role": "assistant",
                            "content": [{
                                "type": "output_text",
                                "text": content
                            }]
                        }));
                    }
                }
                "tool" => {
                    // Tool result
                    let tool_call_id = msg
                        .get("tool_call_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    input_items.push(serde_json::json!({
                        "type": "function_call_output",
                        "call_id": tool_call_id,
                        "output": content
                    }));
                }
                _ => {}
            }
        }

        (instructions, input_items)
    }

    /// Convert tools to Responses API format.
    fn tools_to_responses_format(tools: &[serde_json::Value]) -> Vec<serde_json::Value> {
        tools
            .iter()
            .filter_map(|tool| {
                tool.get("function").map(|func| {
                    serde_json::json!({
                        "type": "function",
                        "name": func.get("name"),
                        "description": func.get("description"),
                        "parameters": func.get("parameters")
                    })
                })
            })
            .collect()
    }
}

/// Request body for Responses API.
#[derive(Serialize)]
struct ResponsesRequest {
    model: String,
    input: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<String>,
    store: bool,
}

/// Response from Responses API.
#[derive(Deserialize, Debug)]
struct ResponsesResponse {
    #[serde(default)]
    output: Vec<ResponsesOutputItem>,
    #[allow(dead_code)]
    #[serde(default)]
    status: String,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
enum ResponsesOutputItem {
    #[serde(rename = "message")]
    Message {
        #[serde(default)]
        role: String,
        #[serde(default)]
        content: Vec<ResponsesContent>,
    },
    #[serde(rename = "function_call")]
    FunctionCall {
        #[serde(default)]
        id: String,
        #[serde(default)]
        call_id: String,
        #[serde(default)]
        name: String,
        #[serde(default)]
        arguments: String,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
enum ResponsesContent {
    #[serde(rename = "output_text")]
    OutputText { text: String },
    #[serde(other)]
    Other,
}

impl AiClient for FoundryClient {
    fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        match self.mode {
            ApiMode::Responses => {
                // Convert chat request to Responses API format
                let (instructions, input) = Self::messages_to_input(&request.messages);
                let tools = Self::tools_to_responses_format(&request.tools);

                let responses_request = ResponsesRequest {
                    model: self.model.clone(),
                    input,
                    instructions,
                    tools,
                    tool_choice: Some("auto".to_string()),
                    store: false,
                };

                // Retry up to 3 times for transient failures
                let mut last_error = None;
                for attempt in 1..=3 {
                    let resp = self
                        .client
                        .post(&self.url)
                        .header("api-key", &self.api_key)
                        .json(&responses_request)
                        .send();

                    match resp {
                        Ok(r) => {
                            if !r.status().is_success() {
                                let status = r.status();
                                let text_body = r
                                    .text()
                                    .unwrap_or_else(|_| "<failed to read error body>".to_string());
                                
                                // Retry on 429 (rate limit) or 5xx errors
                                if status.as_u16() == 429 || status.is_server_error() {
                                    eprintln!("[FoundryClient] Attempt {}/3 failed: HTTP {} - retrying...", attempt, status);
                                    last_error = Some(anyhow::anyhow!(
                                        "Foundry responses request failed: HTTP {} - {}",
                                        status,
                                        text_body
                                    ));
                                    std::thread::sleep(std::time::Duration::from_secs(attempt as u64));
                                    continue;
                                }
                                
                                anyhow::bail!(
                                    "Foundry responses request failed: HTTP {} - {}",
                                    status,
                                    text_body
                                );
                            }
                            
                            // Success - parse response
                            let raw_text = r.text().context("failed to read response body")?;
                            if std::env::var("FOUNDRY_DEBUG").is_ok() {
                                eprintln!("[FoundryClient] Raw response: {}", &raw_text[..raw_text.len().min(500)]);
                            }
                            
                            let parsed: ResponsesResponse = serde_json::from_str(&raw_text)
                                .context("failed to parse Foundry responses JSON")?;

                            // Convert Responses API output to ChatResponse format
                            let mut content_text = String::new();
                            let mut tool_calls = Vec::new();

                            for item in parsed.output {
                                match item {
                                    ResponsesOutputItem::Message { content, .. } => {
                                        for c in content {
                                            if let ResponsesContent::OutputText { text } = c {
                                                content_text.push_str(&text);
                                            }
                                        }
                                    }
                                    ResponsesOutputItem::FunctionCall {
                                        id,
                                        call_id,
                                        name,
                                        arguments,
                                    } => {
                                        tool_calls.push(ChatToolCall {
                                            id: if id.is_empty() { call_id } else { id },
                                            call_type: "function".to_string(),
                                            function: ChatToolFunction { name, arguments },
                                        });
                                    }
                                    ResponsesOutputItem::Unknown => {}
                                }
                            }

                            return Ok(ChatResponse {
                                choices: vec![crate::ai_client::ChatChoice {
                                    message: ChatMessage {
                                        role: "assistant".to_string(),
                                        content: if content_text.is_empty() {
                                            None
                                        } else {
                                            Some(content_text)
                                        },
                                        tool_calls: if tool_calls.is_empty() {
                                            None
                                        } else {
                                            Some(tool_calls)
                                        },
                                    },
                                }],
                            });
                        }
                        Err(e) => {
                            eprintln!("[FoundryClient] Attempt {}/3 network error: {} - retrying...", attempt, e);
                            last_error = Some(anyhow::anyhow!("failed to send Foundry responses request: {}", e));
                            std::thread::sleep(std::time::Duration::from_secs(attempt as u64));
                            continue;
                        }
                    }
                }
                
                Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Foundry responses request failed after retries")))
            }

            ApiMode::ChatCompletions => {
                // Standard chat completions
                let resp = self
                    .client
                    .post(&self.url)
                    .header("api-key", &self.api_key)
                    .json(&request)
                    .send()
                    .context("failed to send Foundry chat request")?;

                if !resp.status().is_success() {
                    let status = resp.status();
                    let text_body = resp
                        .text()
                        .unwrap_or_else(|_| "<failed to read error body>".to_string());
                    anyhow::bail!(
                        "Foundry chat request failed: HTTP {} - {}",
                        status,
                        text_body
                    );
                }

                let parsed: ChatResponse = resp
                    .json()
                    .context("failed to parse Foundry chat response JSON")?;

                Ok(parsed)
            }
        }
    }
}
