// crates/core/src/foundry_client.rs

//! Azure AI Foundry client for the Responses API.

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::ai_client::{AiClient, InputItem, Response, ResponseItem, Tool};

/// Azure AI Foundry client using the Responses API.
///
/// Environment variables:
/// - FOUNDRY_ENDPOINT: e.g. "https://myresource.openai.azure.com"
/// - FOUNDRY_DEPLOYMENT: e.g. "gpt-4o" or "codex"
/// - FOUNDRY_API_KEY: your API key
pub struct FoundryClient {
    client: Client,
    url: String,
    api_key: String,
    model: String,
}

impl FoundryClient {
    pub fn new(endpoint: &str, deployment: &str, api_key: &str) -> Self {
        let url = format!(
            "{}/openai/responses?api-version=2025-03-01-preview",
            endpoint.trim_end_matches('/')
        );

        Self {
            client: Client::new(),
            url,
            api_key: api_key.to_string(),
            model: deployment.to_string(),
        }
    }

    pub fn from_env() -> Result<Self> {
        let endpoint = std::env::var("FOUNDRY_ENDPOINT").context("FOUNDRY_ENDPOINT not set")?;
        let deployment =
            std::env::var("FOUNDRY_DEPLOYMENT").context("FOUNDRY_DEPLOYMENT not set")?;
        let api_key = std::env::var("FOUNDRY_API_KEY").context("FOUNDRY_API_KEY not set")?;

        eprintln!("[FoundryClient] Using Responses API: {}", deployment);

        Ok(Self::new(&endpoint, &deployment, &api_key))
    }
}

/// Request body for Responses API.
#[derive(Serialize)]
struct ResponsesRequest {
    model: String,
    input: Vec<InputItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<Tool>,
    tool_choice: String,
    store: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
}

/// Response from Responses API.
#[derive(Deserialize, Debug)]
struct ResponsesResponse {
    #[serde(default)]
    output: Vec<ResponsesOutputItem>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
enum ResponsesOutputItem {
    #[serde(rename = "message")]
    Message {
        #[serde(default)]
        content: Vec<ResponsesContent>,
    },
    #[serde(rename = "function_call")]
    FunctionCall {
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
    fn respond(
        &self,
        instructions: &str,
        input: Vec<InputItem>,
        tools: &[Tool],
    ) -> Result<Response> {
        // Limit output tokens to avoid rate limits (env override available)
        let max_tokens: u32 = std::env::var("FOUNDRY_MAX_TOKENS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(16000);

        let request = ResponsesRequest {
            model: self.model.clone(),
            input,
            instructions: Some(instructions.to_string()),
            tools: tools.to_vec(),
            tool_choice: "auto".to_string(),
            store: false,
            max_output_tokens: Some(max_tokens),
        };

        // Retry up to 3 times
        let mut last_error = None;

        // Debug: log request on first attempt or if FOUNDRY_DEBUG is set
        if std::env::var("FOUNDRY_DEBUG").is_ok() {
            eprintln!("[FoundryClient] URL: {}", self.url);
            eprintln!("[FoundryClient] Model: {}", self.model);
            if let Ok(json) = serde_json::to_string_pretty(&request) {
                eprintln!(
                    "[FoundryClient] Request:\n{}",
                    &json[..json.len().min(2000)]
                );
            }
        }

        for attempt in 1..=3 {
            let resp = self
                .client
                .post(&self.url)
                .header("api-key", &self.api_key)
                .json(&request)
                .send();

            match resp {
                Ok(r) => {
                    if !r.status().is_success() {
                        let status = r.status();
                        let body = r.text().unwrap_or_default();

                        if status.as_u16() == 429 || status.is_server_error() {
                            // Exponential backoff: 5s, 15s, 30s for rate limits
                            let delay = if status.as_u16() == 429 {
                                5 * attempt as u64 * attempt as u64
                            } else {
                                attempt as u64 * 2
                            };
                            eprintln!(
                                "[FoundryClient] Attempt {}/3: HTTP {} - waiting {}s...\nBody: {}",
                                attempt,
                                status,
                                delay,
                                &body[..body.len().min(500)]
                            );
                            last_error = Some(anyhow::anyhow!("HTTP {} - {}", status, body));
                            std::thread::sleep(std::time::Duration::from_secs(delay));
                            continue;
                        }

                        anyhow::bail!("Foundry request failed: HTTP {} - {}", status, body);
                    }

                    let raw_text = r.text().context("failed to read response body")?;

                    if std::env::var("FOUNDRY_DEBUG").is_ok() {
                        eprintln!(
                            "[FoundryClient] Response: {}",
                            &raw_text[..raw_text.len().min(500)]
                        );
                    }

                    let parsed: ResponsesResponse = serde_json::from_str(&raw_text)
                        .context("failed to parse Foundry response")?;

                    // Convert to our Response type
                    let mut items = Vec::new();

                    for item in parsed.output {
                        match item {
                            ResponsesOutputItem::Message { content } => {
                                let text: String = content
                                    .into_iter()
                                    .filter_map(|c| match c {
                                        ResponsesContent::OutputText { text } => Some(text),
                                        _ => None,
                                    })
                                    .collect();
                                if !text.is_empty() {
                                    items.push(ResponseItem::Message(text));
                                }
                            }
                            ResponsesOutputItem::FunctionCall {
                                call_id,
                                name,
                                arguments,
                            } => {
                                items.push(ResponseItem::FunctionCall {
                                    call_id,
                                    name,
                                    arguments,
                                });
                            }
                            ResponsesOutputItem::Unknown => {}
                        }
                    }

                    return Ok(Response { items });
                }
                Err(e) => {
                    eprintln!(
                        "[FoundryClient] Attempt {}/3 network error: {} - retrying...",
                        attempt, e
                    );
                    last_error = Some(anyhow::anyhow!("network error: {}", e));
                    std::thread::sleep(std::time::Duration::from_secs(attempt as u64));
                    continue;
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("request failed after retries")))
    }
}
