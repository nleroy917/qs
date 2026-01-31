//! Text extraction from files
//!
//! Uses tree-sitter for intelligent code parsing when available,
//! falls back to simple text chunking for unsupported file types.

use std::path::Path;

use crate::parse::{CodeLanguage, CodeParser};
use crate::{Config, Result};

/// Known text file extensions
const TEXT_EXTENSIONS: &[&str] = &[
    // Plain text
    "txt", "md", "rst", "org", "adoc",
    // Code
    "rs", "py", "js", "ts", "jsx", "tsx", "go", "java", "c", "cpp", "h", "hpp",
    "cs", "rb", "php", "swift", "kt", "scala", "hs", "ml", "ex", "exs", "erl",
    "clj", "cljs", "lisp", "scm", "lua", "r", "jl", "nim", "zig", "v", "d",
    // Web
    "html", "htm", "css", "scss", "sass", "less", "vue", "svelte",
    // Config
    "json", "yaml", "yml", "toml", "xml", "ini", "cfg", "conf",
    // Shell
    "sh", "bash", "zsh", "fish", "ps1", "bat", "cmd",
    // Data
    "csv", "sql",
    // Docs
    "tex", "bib",
];

/// Check if a file extension indicates a text file.
pub fn is_text_extension(ext: &str) -> bool {
    TEXT_EXTENSIONS.contains(&ext.to_lowercase().as_str())
}

/// Check if a file should be indexed based on config and extension.
pub fn should_index(path: &Path, config: &Config) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // Check exclude list first
    if config
        .exclude_extensions
        .iter()
        .any(|e| e.to_lowercase() == ext)
    {
        return false;
    }

    // If include list is specified, only include those
    if !config.include_extensions.is_empty() {
        return config
            .include_extensions
            .iter()
            .any(|e| e.to_lowercase() == ext);
    }

    // Default: check if it's a known text extension
    is_text_extension(&ext)
}

/// Extract text content from a file.
///
/// For now, this just reads the file as UTF-8 text.
/// Future: Add support for PDFs, Office docs, etc.
pub fn extract_text(path: &Path) -> Result<String> {
    let content = std::fs::read_to_string(path)?;
    Ok(content)
}

/// A chunk of text with metadata.
#[derive(Debug, Clone)]
pub struct Chunk {
    /// The text content
    pub text: String,
    /// Starting line number (1-indexed)
    pub start_line: usize,
    /// Ending line number (1-indexed)
    pub end_line: usize,
    /// Chunk index within the file
    pub index: usize,
}

/// Extract chunks from a file using the best available method.
///
/// For supported code languages, uses tree-sitter to extract semantic units
/// (functions, classes, structs, etc.). Falls back to simple text chunking
/// for unsupported languages or plain text files.
pub fn extract_chunks(
    path: &Path,
    text: &str,
    chunk_size: usize,
    overlap: usize,
    parser: &mut CodeParser,
) -> Vec<Chunk> {
    // Try tree-sitter parsing for code files
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        if CodeLanguage::from_extension(ext).is_some() {
            if let Some(chunks) = parser.parse_file(path, text) {
                // If tree-sitter extracted chunks, use them
                // But if any chunk is too large, split it further
                let mut result = Vec::new();
                for chunk in chunks {
                    if chunk.text.len() > chunk_size * 2 {
                        // Split large chunks (e.g., huge functions)
                        let sub_chunks = chunk_text(&chunk.text, chunk_size, overlap);
                        for mut sub in sub_chunks {
                            // Adjust line numbers relative to parent
                            sub.start_line += chunk.start_line - 1;
                            sub.end_line = sub.start_line
                                + sub.text.matches('\n').count();
                            sub.index = result.len();
                            result.push(sub);
                        }
                    } else {
                        result.push(Chunk {
                            index: result.len(),
                            ..chunk
                        });
                    }
                }
                if !result.is_empty() {
                    return result;
                }
            }
        }
    }

    // Fall back to simple text chunking
    chunk_text(text, chunk_size, overlap)
}

/// Split text into chunks with overlap (fallback for non-code files).
pub fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<Chunk> {
    if text.is_empty() {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut char_pos = 0;
    let mut chunk_index = 0;

    while char_pos < text.len() {
        let start_pos = char_pos;
        let end_pos = (char_pos + chunk_size).min(text.len());

        // Find the actual end position (try to break at line boundary)
        let chunk_end = if end_pos < text.len() {
            // Look for a newline near the end
            text[start_pos..end_pos]
                .rfind('\n')
                .map(|p| start_pos + p + 1)
                .unwrap_or(end_pos)
        } else {
            end_pos
        };

        let chunk_text = &text[start_pos..chunk_end];

        // Calculate line numbers
        let start_line = text[..start_pos].matches('\n').count() + 1;
        let end_line = text[..chunk_end].matches('\n').count() + 1;

        chunks.push(Chunk {
            text: chunk_text.to_string(),
            start_line,
            end_line,
            index: chunk_index,
        });

        // Move position forward, accounting for overlap
        if chunk_end >= text.len() {
            break;
        }

        char_pos = if overlap < chunk_end - start_pos {
            chunk_end - overlap
        } else {
            chunk_end
        };
        chunk_index += 1;
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_text_extension() {
        assert!(is_text_extension("rs"));
        assert!(is_text_extension("RS"));
        assert!(is_text_extension("py"));
        assert!(is_text_extension("md"));
        assert!(!is_text_extension("png"));
        assert!(!is_text_extension("exe"));
    }

    #[test]
    fn test_chunk_text() {
        let text = "line1\nline2\nline3\nline4\nline5\n";
        let chunks = chunk_text(text, 12, 4);

        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].start_line, 1);
    }

    #[test]
    fn test_chunk_empty() {
        let chunks = chunk_text("", 100, 10);
        assert!(chunks.is_empty());
    }
}