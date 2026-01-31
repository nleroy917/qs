//! Search functionality

use std::path::PathBuf;

use crate::{embed::Embedder, storage::SearchResult, Config, Result, Storage};

/// Searcher for querying the index.
pub struct Searcher {
    embedder: Embedder,
    storage: Storage,
}

impl Searcher {
    /// Create a new searcher for a qs repository.
    pub fn new(root: PathBuf) -> Result<Self> {
        let config = Config::load(&root)?;
        let embedder = Embedder::new(&config)?;
        let storage = Storage::open(&root, &config)?;

        Ok(Self { embedder, storage })
    }

    /// Search for chunks matching the query.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        // Embed the query
        let query_embedding = self.embedder.embed(query)?;

        // Search storage
        self.storage.search(query_embedding, limit)
    }

    /// Find chunks similar to a given file.
    pub fn similar(&self, file_path: &std::path::Path, limit: usize) -> Result<Vec<SearchResult>> {
        // Read and embed the file content
        let content = std::fs::read_to_string(file_path)?;
        let embedding = self.embedder.embed(&content)?;

        // Search storage
        self.storage.search(embedding, limit)
    }
}