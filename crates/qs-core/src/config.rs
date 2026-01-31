//! Configuration handling for .qs/config.json

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{
    discover, Result, DEFAULT_CHUNK_OVERLAP, DEFAULT_CHUNK_SIZE, DEFAULT_DIM, DEFAULT_MAX_FILE_SIZE,
    DEFAULT_MODEL,
};

/// Configuration stored in .qs/config.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Embedding model name (fastembed model ID)
    #[serde(default = "default_model")]
    pub model: String,

    /// Embedding dimension
    #[serde(default = "default_dim")]
    pub dimension: usize,

    /// Chunk size in characters
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,

    /// Chunk overlap in characters
    #[serde(default = "default_chunk_overlap")]
    pub chunk_overlap: usize,

    /// Maximum file size to index (bytes)
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,

    /// File extensions to include (empty = all text files)
    #[serde(default)]
    pub include_extensions: Vec<String>,

    /// File extensions to exclude
    #[serde(default)]
    pub exclude_extensions: Vec<String>,

    /// Additional paths to ignore (on top of .gitignore)
    #[serde(default)]
    pub ignore_paths: Vec<String>,
}

fn default_model() -> String {
    DEFAULT_MODEL.to_string()
}

fn default_dim() -> usize {
    DEFAULT_DIM
}

fn default_chunk_size() -> usize {
    DEFAULT_CHUNK_SIZE
}

fn default_chunk_overlap() -> usize {
    DEFAULT_CHUNK_OVERLAP
}

fn default_max_file_size() -> u64 {
    DEFAULT_MAX_FILE_SIZE
}

impl Default for Config {
    fn default() -> Self {
        Self {
            model: default_model(),
            dimension: default_dim(),
            chunk_size: default_chunk_size(),
            chunk_overlap: default_chunk_overlap(),
            max_file_size: default_max_file_size(),
            include_extensions: Vec::new(),
            exclude_extensions: Vec::new(),
            ignore_paths: Vec::new(),
        }
    }
}

impl Config {
    /// Load config from the .qs directory.
    pub fn load(root: &Path) -> Result<Self> {
        let path = discover::config_path(root);
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&content)?)
        } else {
            Ok(Self::default())
        }
    }

    /// Save config to the .qs directory.
    pub fn save(&self, root: &Path) -> Result<()> {
        let path = discover::config_path(root);
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}