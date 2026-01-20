// crates/core/src/capability_index.rs
use std::collections::HashMap;

use anyhow::{Context, Result};

use crate::embedding::Embedder;
use crate::types::CapabilityRecord;

/// In-memory index from capability id -> embedding.
///
/// Uses linear scan + cosine similarity. That's fine for an MVP.
#[derive(Debug)]
pub struct CapabilityIndex {
    dim: usize,
    embeddings: HashMap<String, Vec<f32>>,
}

impl CapabilityIndex {
    /// Build an index from a set of capabilities, embedding any missing ones.
    pub fn build<E: Embedder>(capabilities: &mut [CapabilityRecord], embedder: &E) -> Result<Self> {
        let mut embeddings = HashMap::new();
        let mut dim: Option<usize> = None;
        let total = capabilities.len();

        for (i, cap) in capabilities.iter_mut().enumerate() {
            if cap.embedding.is_none() {
                eprintln!(
                    "[index] ({}/{}) Embedding capability: {} ...",
                    i + 1,
                    total,
                    cap.id
                );
                let emb = embedder.embed(&cap.summary).with_context(|| {
                    format!("failed to embed summary for capability {}", cap.id)
                })?;
                cap.embedding = Some(emb);
                eprintln!("[index] ({}/{}) Done: {}", i + 1, total, cap.id);
            } else {
                eprintln!(
                    "[index] ({}/{}) Using cached embedding: {}",
                    i + 1,
                    total,
                    cap.id
                );
            }

            let emb = cap.embedding.as_ref().unwrap();
            if let Some(d) = dim {
                if d != emb.len() {
                    anyhow::bail!(
                        "inconsistent embedding dimensions: {} vs {} for capability {}",
                        d,
                        emb.len(),
                        cap.id
                    );
                }
            } else {
                dim = Some(emb.len());
            }

            embeddings.insert(cap.id.clone(), emb.clone());
        }

        Ok(Self {
            dim: dim.unwrap_or(0),
            embeddings,
        })
    }

    pub fn len(&self) -> usize {
        self.embeddings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.embeddings.is_empty()
    }

    /// Embed a task description and return top-k capability ids with scores.
    pub fn nearest_for_task<E: Embedder>(
        &self,
        task_description: &str,
        embedder: &E,
        k: usize,
    ) -> Result<Vec<(String, f32)>> {
        let query_emb = embedder
            .embed(task_description)
            .context("failed to embed task description")?;

        if self.dim != query_emb.len() {
            anyhow::bail!(
                "query embedding dimension {} does not match index dimension {}",
                query_emb.len(),
                self.dim
            );
        }

        Ok(self.nearest_from_embedding(&query_emb, k))
    }

    /// Given a precomputed query embedding, return top-k (capability_id, score).
    pub fn nearest_from_embedding(&self, query_emb: &[f32], k: usize) -> Vec<(String, f32)> {
        let mut scored: Vec<(String, f32)> = self
            .embeddings
            .iter()
            .map(|(id, emb)| {
                let score = cosine_similarity(query_emb, emb);
                (id.clone(), score)
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);
        scored
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;

    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }

    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }

    dot / (na.sqrt() * nb.sqrt())
}
