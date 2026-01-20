// crates/core/src/embedding.rs

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Abstract embedding provider.
///
/// Implementations can use Microsoft AI Foundry (Azure OpenAI), local models, etc.
/// For now we only provide a MicrosoftFoundryEmbedder.
pub trait Embedder {
    fn embed(&self, text: &str) -> Result<Vec<f32>>;
}

/// Embedding client for Microsoft AI Foundry (Azure OpenAI).
///
/// Expects the following environment variables:
///
/// - FOUNDRY_ENDPOINT
///     e.g. "https://myresource.openai.azure.com"
///
/// - FOUNDRY_EMBED_DEPLOYMENT
///     e.g. "text-embedding-3-small"
///
/// - FOUNDRY_API_KEY
///     your Azure OpenAI / Foundry API key
///
/// - FOUNDRY_API_VERSION (optional)
///     default: "2024-02-15-preview"
pub struct MicrosoftFoundryEmbedder {
    endpoint: String,
    deployment: String,
    api_key: String,
    api_version: String,
}

impl MicrosoftFoundryEmbedder {
    /// Construct from environment variables.
    pub fn from_env() -> Result<Self> {
        let endpoint = std::env::var("FOUNDRY_ENDPOINT").context("FOUNDRY_ENDPOINT not set")?;

        let deployment = std::env::var("FOUNDRY_EMBED_DEPLOYMENT")
            .context("FOUNDRY_EMBED_DEPLOYMENT not set")?;

        let api_key = std::env::var("FOUNDRY_API_KEY").context("FOUNDRY_API_KEY not set")?;

        let api_version = std::env::var("FOUNDRY_API_VERSION")
            .unwrap_or_else(|_| "2024-02-15-preview".to_string());

        Ok(Self {
            endpoint,
            deployment,
            api_key,
            api_version,
        })
    }
}

#[derive(Debug, Serialize)]
struct FoundryEmbeddingRequest<'a> {
    input: &'a str,
}

#[derive(Debug, Deserialize)]
struct FoundryEmbeddingResponse {
    data: Vec<FoundryEmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct FoundryEmbeddingData {
    embedding: Vec<f32>,
}

impl Embedder for MicrosoftFoundryEmbedder {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let client = reqwest::blocking::Client::new();

        // Azure / Foundry embedding endpoint shape:
        // POST {endpoint}/openai/deployments/{deployment}/embeddings?api-version={version}
        let url = format!(
            "{}/openai/deployments/{}/embeddings?api-version={}",
            self.endpoint.trim_end_matches('/'),
            self.deployment,
            self.api_version,
        );

        let body = FoundryEmbeddingRequest { input: text };

        let resp = client
            .post(&url)
            .header("api-key", &self.api_key)
            .json(&body)
            .send()
            .context("failed to send Microsoft Foundry embedding request")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text_body = resp
                .text()
                .unwrap_or_else(|_| "<failed to read error body>".to_string());
            anyhow::bail!(
                "Microsoft Foundry embeddings request failed: HTTP {} - {}",
                status,
                text_body
            );
        }

        let parsed: FoundryEmbeddingResponse = resp
            .json()
            .context("failed to parse Microsoft Foundry embeddings response JSON")?;

        let first = parsed
            .data
            .into_iter()
            .next()
            .context("Microsoft Foundry embeddings response contained no data")?;

        Ok(first.embedding)
    }
}
