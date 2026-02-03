# qs - Semantic Filesystem Search

A CLI tool for semantically searching your codebase using local vector embeddings and [qdrant-edge](https://qdrant.tech/edge/). Like `grep`, but understands meaning.

```bash
$ qs "function that parses tree-sitter AST"

[1] 0.847  src/parse.rs:45-89
│   45 │ pub fn parse_file(&mut self, path: &Path, source: &str) -> Option<Vec<Chunk>> {
│   46 │     let ext = path.extension()?.to_str()?;
│   47 │     let lang = CodeLanguage::from_extension(ext)?;
     ┊  ... 38 more lines ...
│   87 │     Some(extract_chunks(&tree, source, lang))
│   88 │ }
```

## Installation

```bash
# Clone and build
git clone https://github.com/nleroy917/qs
cd qs
cargo install --path crates/qs-cli --features full
```

Requires Rust 1.85+.

## Usage

```bash
# Initialize in your project
cd your-project
qs init

# Index your codebase
qs index

# Search!
qs "error handling for network requests"
qs "database connection pooling"
qs "parse JSON response"

# Find similar files
qs similar src/auth.rs

# Check index status
qs status
```

### Commands

| Command | Description |
|---------|-------------|
| `qs init` | Initialize `.qs` folder in current directory |
| `qs index [path]` | Index files (respects `.gitignore`) |
| `qs <query>` | Semantic search |
| `qs search <query> -n 20` | Search with custom result limit |
| `qs similar <file>` | Find files similar to a given file |
| `qs status` | Show index statistics |
| `qs update` | Re-index changed files |

### Options

```bash
qs search "query" -n 20      # Return 20 results (default: 10)
qs search "query" -C 5       # Show 5 context lines (default: 2)
```

## How It Works

1. **Parsing** - [tree-sitter](https://tree-sitter.github.io/) parses code into AST, extracting semantic units (functions, classes, etc.)
2. **Embedding** - [fastembed](https://github.com/Anush008/fastembed-rs) generates embeddings using `jina-embeddings-v2-base-code` (optimized for code)
3. **Storage** - [Qdrant Edge](https://github.com/qdrant/qdrant) stores vectors locally in `.qs/shard/`
4. **Search** - Query is embedded and matched against stored vectors using cosine similarity

### Supported Languages

Tree-sitter parsing: Rust, Python, JavaScript, TypeScript, Go, Java, C, C++

Other text files fall back to character-based chunking.

## Configuration

Edit `.qs/config.json`:

```json
{
  "model": "jina-embeddings-v2-base-code",
  "dimension": 768,
  "chunk_size": 2000,
  "chunk_overlap": 200,
  "max_file_size": 1048576,
  "exclude_extensions": ["min.js", "map"],
  "include_extensions": []
}
```

## Storage

All data stored locally in `.qs/`:

```
.qs/
├── config.json     # Configuration
├── files.json      # File metadata & hashes
└── shard/          # Qdrant Edge vector storage
    ├── wal/
    └── segments/
```

## License

MIT
