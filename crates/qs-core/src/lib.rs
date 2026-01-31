//! qs-core: Semantic filesystem search library
//!
//! This library provides the core functionality for indexing and searching
//! local files using vector embeddings stored in Qdrant Edge.

pub mod config;
pub mod discover;
pub mod embed;
pub mod extract;
pub mod index;
pub mod parse;
pub mod search;
pub mod storage;

pub use config::Config;
pub use discover::find_qs_root;
pub use index::Indexer;
pub use search::Searcher;
pub use storage::Storage;

/// The name of the qs folder (like .git)
pub const QS_DIR: &str = ".qs";

/// Default embedding model (code-optimized)
pub const DEFAULT_MODEL: &str = "jina-embeddings-v2-base-code";

/// Default embedding dimension for jina-embeddings-v2-base-code
pub const DEFAULT_DIM: usize = 768;

/// Default chunk size in characters (roughly ~512 tokens)
pub const DEFAULT_CHUNK_SIZE: usize = 2000;

/// Default chunk overlap in characters
pub const DEFAULT_CHUNK_OVERLAP: usize = 200;

/// Default max file size (1MB)
pub const DEFAULT_MAX_FILE_SIZE: u64 = 1024 * 1024;

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