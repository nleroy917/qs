//! Indexing logic: walk files, extract text, chunk, embed, store

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};

use crate::{
    discover, embed::Embedder, extract, parse::CodeParser, storage::ChunkPayload, Config, Result,
    Storage,
};

/// Progress events emitted during indexing.
#[derive(Debug, Clone)]
pub enum ProgressEvent<'a> {
    /// Scanning for files to index.
    Scanning { count: usize },
    /// Indexing a specific file.
    Indexing {
        current: usize,
        total: usize,
        path: &'a Path,
    },
    /// Generating embeddings.
    Embedding { current: usize, total: usize },
}

/// Type alias for progress callback.
pub type ProgressCallback = Box<dyn Fn(ProgressEvent) + Send>;

/// Metadata about an indexed file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    /// Blake3 hash of file content
    pub hash: String,
    /// Last modification time (unix timestamp)
    pub mtime: u64,
    /// Number of chunks
    pub chunk_count: usize,
    /// Starting point ID for this file's chunks
    pub start_id: u64,
}

/// File index stored in .qs/files.json
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct FileIndex {
    /// Map of relative path -> metadata
    pub files: HashMap<String, FileMetadata>,
    /// Next available point ID
    pub next_id: u64,
}

impl FileIndex {
    /// Load from disk.
    pub fn load(root: &Path) -> Result<Self> {
        let path = discover::files_path(root);
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&content)?)
        } else {
            Ok(Self::default())
        }
    }

    /// Save to disk.
    pub fn save(&self, root: &Path) -> Result<()> {
        let path = discover::files_path(root);
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

/// The indexer orchestrates file discovery, embedding, and storage.
pub struct Indexer {
    root: PathBuf,
    config: Config,
    embedder: Embedder,
    storage: Storage,
    file_index: FileIndex,
    parser: CodeParser,
    progress_callback: Option<ProgressCallback>,
}

/// Stats from an indexing run.
#[derive(Debug, Default)]
pub struct IndexStats {
    pub files_scanned: usize,
    pub files_indexed: usize,
    pub files_skipped: usize,
    pub files_unchanged: usize,
    pub chunks_created: usize,
}

impl Indexer {
    /// Create a new indexer for a qs repository.
    pub fn new(root: PathBuf) -> Result<Self> {
        let config = Config::load(&root)?;
        let embedder = Embedder::new(&config)?;
        let storage = Storage::open(&root, &config)?;
        let file_index = FileIndex::load(&root)?;
        let parser = CodeParser::new();

        Ok(Self {
            root,
            config,
            embedder,
            storage,
            file_index,
            parser,
            progress_callback: None,
        })
    }

    /// Set a callback to receive progress updates during indexing.
    pub fn set_progress_callback(&mut self, callback: ProgressCallback) {
        self.progress_callback = Some(callback);
    }

    /// Emit a progress event if a callback is registered.
    fn emit_progress(&self, event: ProgressEvent) {
        if let Some(ref callback) = self.progress_callback {
            callback(event);
        }
    }

    /// Index files in the given path (relative to repo root).
    pub fn index(&mut self, path: Option<&Path>) -> Result<IndexStats> {
        let start_path = match path {
            Some(p) => self.root.join(p),
            None => self.root.clone(),
        };

        let mut stats = IndexStats::default();

        // Walk files, respecting .gitignore
        let walker = WalkBuilder::new(&start_path)
            .hidden(true) // Skip hidden files
            .git_ignore(true) // Respect .gitignore
            .git_global(true)
            .git_exclude(true)
            .build();

        let mut files_to_index: Vec<(PathBuf, String)> = Vec::new();

        for entry in walker.flatten() {
            let path = entry.path();

            // Skip directories
            if path.is_dir() {
                continue;
            }

            // Skip the .qs folder
            if path.starts_with(discover::qs_dir(&self.root)) {
                continue;
            }

            stats.files_scanned += 1;
            self.emit_progress(ProgressEvent::Scanning {
                count: stats.files_scanned,
            });

            // Check if we should index this file type
            if !extract::should_index(path, &self.config) {
                stats.files_skipped += 1;
                continue;
            }

            // Check file size
            let metadata = std::fs::metadata(path)?;
            if metadata.len() > self.config.max_file_size {
                stats.files_skipped += 1;
                continue;
            }

            // Calculate file hash
            let content = match std::fs::read(path) {
                Ok(c) => c,
                Err(_) => {
                    stats.files_skipped += 1;
                    continue;
                }
            };
            let hash = blake3::hash(&content).to_hex().to_string();

            // Get relative path
            let rel_path = path
                .strip_prefix(&self.root)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();

            // Check if file has changed
            if let Some(existing) = self.file_index.files.get(&rel_path) {
                if existing.hash == hash {
                    stats.files_unchanged += 1;
                    continue;
                }

                // File changed - delete old chunks
                let ids_to_delete: Vec<u64> =
                    (existing.start_id..existing.start_id + existing.chunk_count as u64).collect();
                self.storage.delete(ids_to_delete)?;
            }

            files_to_index.push((path.to_path_buf(), hash));
        }

        // Process files
        let total_files = files_to_index.len();
        for (i, (path, hash)) in files_to_index.iter().enumerate() {
            self.emit_progress(ProgressEvent::Indexing {
                current: i + 1,
                total: total_files,
                path,
            });

            match self.index_file(path, hash) {
                Ok(chunk_count) => {
                    stats.files_indexed += 1;
                    stats.chunks_created += chunk_count;
                }
                Err(e) => {
                    tracing::warn!("Failed to index {}: {}", path.display(), e);
                    stats.files_skipped += 1;
                }
            }
        }

        // Save file index
        self.file_index.save(&self.root)?;

        // Flush storage
        self.storage.flush();

        Ok(stats)
    }

    /// Index a single file.
    fn index_file(&mut self, path: &Path, hash: &str) -> Result<usize> {
        let rel_path = path
            .strip_prefix(&self.root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        // Extract text
        let text = extract::extract_text(path)?;
        if text.is_empty() {
            return Ok(0);
        }

        // Extract chunks using tree-sitter for code files, text chunking for others
        let chunks = extract::extract_chunks(
            path,
            &text,
            self.config.chunk_size,
            self.config.chunk_overlap,
            &mut self.parser,
        );
        if chunks.is_empty() {
            return Ok(0);
        }

        // Generate embeddings
        let texts: Vec<&str> = chunks.iter().map(|c| c.text.as_str()).collect();
        let embeddings = self.embedder.embed_batch(&texts)?;

        // Prepare points for storage
        let start_id = self.file_index.next_id;
        let mut points = Vec::with_capacity(chunks.len());

        for (i, (chunk, embedding)) in chunks.iter().zip(embeddings.into_iter()).enumerate() {
            let point_id = start_id + i as u64;
            let payload = ChunkPayload {
                path: rel_path.clone(),
                chunk_index: chunk.index,
                start_line: chunk.start_line,
                end_line: chunk.end_line,
                text: chunk.text.clone(),
                file_hash: hash.to_string(),
            };
            points.push((point_id, embedding, payload));
        }

        // Store vectors
        self.storage.upsert(points)?;

        // Update file index
        self.file_index.files.insert(
            rel_path,
            FileMetadata {
                hash: hash.to_string(),
                mtime: std::fs::metadata(path)
                    .map(|m| {
                        m.modified()
                            .ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_secs())
                            .unwrap_or(0)
                    })
                    .unwrap_or(0),
                chunk_count: chunks.len(),
                start_id,
            },
        );
        self.file_index.next_id = start_id + chunks.len() as u64;

        Ok(chunks.len())
    }

    /// Get the current storage count.
    pub fn count(&self) -> Result<usize> {
        self.storage.count()
    }
}