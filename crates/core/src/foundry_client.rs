// crates/core/src/foundry_client.rs

use anyhow::{Context, Result};
use reqwest::blocking::Client;

use crate::ai_client::{AiClient, ChatRequest, ChatResponse};

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
}

impl FoundryClient {
    /// Construct with explicit parameters.
    pub fn new(endpoint: &str, deployment: &str, api_key: &str, api_version: Option<&str>) -> Self {
        let api_version = api_version.unwrap_or("2024-02-15-preview");
        let url = format!(
            "{}/openai/deployments/{}/chat/completions?api-version={}",
            endpoint.trim_end_matches('/'),
            deployment,
            api_version,
        );

        Self {
            client: Client::new(),
            url,
            api_key: api_key.to_string(),
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
    pub fn from_env_with_deployment_var(deployment_var: &str) -> Result<Self> {
        let endpoint = std::env::var("FOUNDRY_ENDPOINT").context("FOUNDRY_ENDPOINT not set")?;

        let deployment =
            std::env::var(deployment_var).with_context(|| format!("{} not set", deployment_var))?;

        let api_key = std::env::var("FOUNDRY_API_KEY").context("FOUNDRY_API_KEY not set")?;

        let api_version = std::env::var("FOUNDRY_API_VERSION")
            .unwrap_or_else(|_| "2024-02-15-preview".to_string());

        Ok(Self::new(
            &endpoint,
            &deployment,
            &api_key,
            Some(&api_version),
        ))
    }
}

impl AiClient for FoundryClient {
    fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
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
