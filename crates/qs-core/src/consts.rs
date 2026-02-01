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