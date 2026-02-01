//! Tree-sitter based code parsing for semantic chunking
//!
//! Extracts meaningful code units (functions, classes, structs, methods)
//! as chunks for embedding.

use std::path::Path;

use tree_sitter::{Language, Parser, Tree};

use crate::extract::Chunk;

/// Supported programming languages for tree-sitter parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeLanguage {
    #[cfg(feature="rs")]
    Rust,
    #[cfg(feature="python")]
    Python,
    #[cfg(feature="javascript")]
    JavaScript,
    #[cfg(feature="typescript")]
    TypeScript,
    #[cfg(feature="go")]
    Go,
    #[cfg(feature="java")]
    Java,
    #[cfg(feature="c")]
    C,
    #[cfg(feature="cpp")]
    Cpp,
}

impl CodeLanguage {
    /// Detect language from file extension.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            #[cfg(feature="rs")]
            "rs" => Some(Self::Rust),
            #[cfg(feature="python")]
            "py" | "pyi" => Some(Self::Python),
            #[cfg(feature="javascript")]
            "js" | "jsx" | "mjs" | "cjs" => Some(Self::JavaScript),
            #[cfg(feature="typescript")]
            "ts" | "tsx" | "mts" | "cts" => Some(Self::TypeScript),
            #[cfg(feature="go")]
            "go" => Some(Self::Go),
            #[cfg(feature="java")]
            "java" => Some(Self::Java),
            #[cfg(feature="c")]
            "c" | "h" => Some(Self::C),
            #[cfg(feature="cpp")]
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" => Some(Self::Cpp),
            _ => None,
        }
    }

    /// Get the tree-sitter language for this code language.
    fn tree_sitter_language(&self) -> Language {
        match self {
            #[cfg(feature="rs")]
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            #[cfg(feature="python")]
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            #[cfg(feature="javascript")]
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            #[cfg(feature="typescript")]
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            #[cfg(feature="go")]
            Self::Go => tree_sitter_go::LANGUAGE.into(),
            #[cfg(feature="java")]
            Self::Java => tree_sitter_java::LANGUAGE.into(),
            #[cfg(feature="c")]
            Self::C => tree_sitter_c::LANGUAGE.into(),
            #[cfg(feature="cpp")]
            Self::Cpp => tree_sitter_cpp::LANGUAGE.into(),
        }
    }

    /// Get the node kinds that represent top-level definitions we want to extract.
    fn definition_kinds(&self) -> &[&str] {
        match self {
            #[cfg(feature="rs")]
            Self::Rust => &[
                "function_item",
                "impl_item",
                "struct_item",
                "enum_item",
                "trait_item",
                "mod_item",
                "const_item",
                "static_item",
                "type_item",
                "macro_definition",
            ],
            #[cfg(feature="python")]
            Self::Python => &[
                "function_definition",
                "class_definition",
                "decorated_definition",
            ],
            #[cfg(any(feature="javascript", feature="typescript"))]
            Self::JavaScript | Self::TypeScript => &[
                "function_declaration",
                "class_declaration",
                "method_definition",
                "arrow_function",
                "function",
                "export_statement",
                "lexical_declaration",
            ],
            #[cfg(feature="go")]
            Self::Go => &[
                "function_declaration",
                "method_declaration",
                "type_declaration",
                "const_declaration",
                "var_declaration",
            ],
            #[cfg(feature="java")]
            Self::Java => &[
                "class_declaration",
                "interface_declaration",
                "enum_declaration",
                "method_declaration",
                "constructor_declaration",
            ],
            #[cfg(any(feature="c", feature="cpp"))]
            Self::C | Self::Cpp => &[
                "function_definition",
                "struct_specifier",
                "enum_specifier",
                "class_specifier",
                "namespace_definition",
            ],
        }
    }
}

/// Code parser using tree-sitter.
pub struct CodeParser {
    parser: Parser,
}

impl CodeParser {
    /// Create a new code parser.
    pub fn new() -> Self {
        Self {
            parser: Parser::new(),
        }
    }

    /// Parse a file and extract semantic chunks.
    ///
    /// Returns `None` if the language is not supported or parsing fails.
    pub fn parse_file(&mut self, path: &Path, source: &str) -> Option<Vec<Chunk>> {
        let ext = path.extension()?.to_str()?;
        let lang = CodeLanguage::from_extension(ext)?;

        self.parser
            .set_language(&lang.tree_sitter_language())
            .ok()?;

        let tree = self.parser.parse(source, None)?;

        Some(extract_chunks(&tree, source, lang))
    }
}

impl Default for CodeParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract chunks from a parsed syntax tree.
fn extract_chunks(tree: &Tree, source: &str, lang: CodeLanguage) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let definition_kinds = lang.definition_kinds();

    let root = tree.root_node();
    let mut cursor = root.walk();

    // Walk through top-level nodes
    for child in root.children(&mut cursor) {
        let kind = child.kind();

        // Check if this is a definition we want to extract
        if definition_kinds.contains(&kind) {
            let start_byte = child.start_byte();
            let end_byte = child.end_byte();
            let text = &source[start_byte..end_byte];

            // Calculate line numbers
            let start_line = source[..start_byte].matches('\n').count() + 1;
            let end_line = source[..end_byte].matches('\n').count() + 1;

            chunks.push(Chunk {
                text: text.to_string(),
                start_line,
                end_line,
                index: chunks.len(),
            });
        }
    }

    // If no chunks extracted (e.g., file has only nested definitions),
    // try extracting from all descendants
    if chunks.is_empty() {
        extract_chunks_recursive(&root, source, definition_kinds, &mut chunks);
    }

    // If still no chunks, fall back to treating the whole file as one chunk
    if chunks.is_empty() && !source.trim().is_empty() {
        chunks.push(Chunk {
            text: source.to_string(),
            start_line: 1,
            end_line: source.matches('\n').count() + 1,
            index: 0,
        });
    }

    chunks
}

/// Recursively extract chunks from nested definitions.
fn extract_chunks_recursive(
    node: &tree_sitter::Node,
    source: &str,
    definition_kinds: &[&str],
    chunks: &mut Vec<Chunk>,
) {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        let kind = child.kind();

        if definition_kinds.contains(&kind) {
            let start_byte = child.start_byte();
            let end_byte = child.end_byte();
            let text = &source[start_byte..end_byte];

            let start_line = source[..start_byte].matches('\n').count() + 1;
            let end_line = source[..end_byte].matches('\n').count() + 1;

            chunks.push(Chunk {
                text: text.to_string(),
                start_line,
                end_line,
                index: chunks.len(),
            });
        } else {
            // Recurse into children
            extract_chunks_recursive(&child, source, definition_kinds, chunks);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_detection() {
        assert_eq!(
            CodeLanguage::from_extension("rs"),
            Some(CodeLanguage::Rust)
        );
        assert_eq!(
            CodeLanguage::from_extension("py"),
            Some(CodeLanguage::Python)
        );
        assert_eq!(
            CodeLanguage::from_extension("ts"),
            Some(CodeLanguage::TypeScript)
        );
        assert_eq!(CodeLanguage::from_extension("txt"), None);
    }

    #[test]
    fn test_parse_rust() {
        let source = r#"
fn hello() {
    println!("Hello");
}

struct Foo {
    x: i32,
}

impl Foo {
    fn new() -> Self {
        Self { x: 0 }
    }
}
"#;

        let mut parser = CodeParser::new();
        let chunks = parser
            .parse_file(Path::new("test.rs"), source)
            .expect("should parse");

        assert_eq!(chunks.len(), 3); // fn, struct, impl
    }

    #[test]
    fn test_parse_python() {
        let source = r#"
def hello():
    print("Hello")

class Foo:
    def __init__(self):
        self.x = 0
"#;

        let mut parser = CodeParser::new();
        let chunks = parser
            .parse_file(Path::new("test.py"), source)
            .expect("should parse");

        assert_eq!(chunks.len(), 2); // def, class
    }
}