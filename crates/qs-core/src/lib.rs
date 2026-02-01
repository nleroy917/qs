//! qs-core: Semantic filesystem search library
//!
//! This library provides the core functionality for indexing and searching
//! local files using vector embeddings stored in Qdrant Edge.

pub mod config;
pub mod consts;
pub mod discover;
pub mod embed;
pub mod extract;
pub mod index;
pub mod parse;
pub mod search;
pub mod storage;

pub use config::Config;
pub use consts::*;
pub use discover::find_qs_root;
pub use index::Indexer;
pub use search::Searcher;
pub use storage::Storage;

#[derive(Debug, thiserror::Error)]
pub enum QsError {
    #[error("Not in a qs repository (no .qs folder found)")]
    NotInRepo,

    #[error("Already initialized: {0}")]
    AlreadyInitialized(std::path::PathBuf),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Embedding error: {0}")]
    Embedding(String),

    #[error("Index error: {0}")]
    Index(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, QsError>;
