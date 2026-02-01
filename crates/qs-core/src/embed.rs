//! Embedding generation using fastembed

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

use crate::{Config, QsError, Result};

/// Wrapper around fastembed for generating embeddings.
pub struct Embedder {
    model: TextEmbedding,
}

impl Embedder {
    /// Create a new embedder with the model specified in config.
    pub fn new(config: &Config) -> Result<Self> {
        let model_type = match config.model.as_str() {
            // Code-optimized model (default)
            "jina-embeddings-v2-base-code" => EmbeddingModel::JinaEmbeddingsV2BaseCode,
            // General-purpose models
            "all-MiniLM-L6-v2" => EmbeddingModel::AllMiniLML6V2,
            "all-MiniLM-L12-v2" => EmbeddingModel::AllMiniLML12V2,
            "bge-small-en-v1.5" => EmbeddingModel::BGESmallENV15,
            "bge-base-en-v1.5" => EmbeddingModel::BGEBaseENV15,
            other => {
                return Err(QsError::Embedding(format!(
                    "Unknown model: {}. Supported: jina-embeddings-v2-base-code, all-MiniLM-L6-v2, bge-small-en-v1.5",
                    other
                )));
            }
        };

        let model =
            TextEmbedding::try_new(InitOptions::new(model_type).with_show_download_progress(true))
                .map_err(|e| QsError::Embedding(e.to_string()))?;

        Ok(Self { model })
    }

    /// Generate embeddings for a batch of texts.
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let embeddings = self
            .model
            .embed(texts.to_vec(), None)
            .map_err(|e| QsError::Embedding(e.to_string()))?;

        Ok(embeddings)
    }

    /// Generate embedding for a single text.
    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let embeddings = self.embed_batch(&[text])?;
        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| QsError::Embedding("No embedding generated".to_string()))
    }
}
