//! qs CLI: Semantic filesystem search

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use qs_core::{discover, Config, Indexer, Searcher, QS_DIR};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};

#[derive(Parser)]
#[command(name = "qs")]
#[command(about = "Semantic filesystem search", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Search query (when no subcommand is given)
    #[arg(trailing_var_arg = true)]
    query: Vec<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new .qs repository
    Init,

    /// Index files in the repository
    Index {
        /// Path to index (default: current directory)
        path: Option<PathBuf>,
    },

    /// Show index status and statistics
    Status,

    /// Re-index changed files
    Update,

    /// Find files similar to the given file
    Similar {
        /// File to find similar files for
        file: PathBuf,

        /// Maximum number of results
        #[arg(short = 'n', long, default_value = "10")]
        limit: usize,
    },

    /// Search for files matching a query
    Search {
        /// Search query
        query: Vec<String>,

        /// Maximum number of results
        #[arg(short = 'n', long, default_value = "10")]
        limit: usize,

        /// Number of context lines to show
        #[arg(short = 'C', long, default_value = "2")]
        context: usize,
    },
}

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Init) => cmd_init()?,
        Some(Commands::Index { path }) => cmd_index(path)?,
        Some(Commands::Status) => cmd_status()?,
        Some(Commands::Update) => cmd_update()?,
        Some(Commands::Similar { file, limit }) => cmd_similar(file, limit)?,
        Some(Commands::Search {
            query,
            limit,
            context,
        }) => {
            let query = query.join(" ");
            cmd_search(&query, limit, context)?;
        }
        None => {
            // Default: search with the provided query
            if cli.query.is_empty() {
                // No query provided, show help
                println!("Usage: qs <query> or qs <command>");
                println!("Run 'qs --help' for more information.");
            } else {
                let query = cli.query.join(" ");
                cmd_search(&query, 10, 2)?;
            }
        }
    }

    Ok(())
}

fn cmd_init() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let qs_dir = cwd.join(QS_DIR);

    if qs_dir.exists() {
        anyhow::bail!("Already initialized: {} exists", qs_dir.display());
    }

    // Create .qs directory
    std::fs::create_dir(&qs_dir)?;

    // Create default config
    let config = Config::default();
    config.save(&cwd)?;

    println!("Initialized qs repository in {}", qs_dir.display());
    println!("Run 'qs index' to index files.");

    Ok(())
}

fn cmd_index(path: Option<PathBuf>) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let root =
        discover::find_qs_root(&cwd).context("Not in a qs repository. Run 'qs init' first.")?;

    // Create progress bar
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    pb.set_message("Scanning files...");

    let mut indexer = Indexer::new(root)?;

    // Set up progress callback
    indexer.set_progress_callback(Box::new({
        let pb = pb.clone();
        move |event| match event {
            qs_core::index::ProgressEvent::Scanning { count } => {
                pb.set_message(format!("Scanning... {} files found", count));
            }
            qs_core::index::ProgressEvent::Indexing { current, total, path } => {
                pb.set_style(
                    ProgressStyle::default_bar()
                        .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                        .unwrap()
                        .progress_chars("█▓░"),
                );
                pb.set_length(total as u64);
                pb.set_position(current as u64);
                pb.set_message(path.to_string_lossy().to_string());
            }
            qs_core::index::ProgressEvent::Embedding { current, total } => {
                pb.set_message(format!("Embedding chunks {}/{}...", current, total));
            }
        }
    }));

    let stats = indexer.index(path.as_deref())?;

    pb.finish_and_clear();

    println!("✓ Indexing complete:");
    println!("  Files scanned:   {}", stats.files_scanned);
    println!("  Files indexed:   {}", stats.files_indexed);
    println!("  Files unchanged: {}", stats.files_unchanged);
    println!("  Files skipped:   {}", stats.files_skipped);
    println!("  Chunks created:  {}", stats.chunks_created);

    Ok(())
}

fn cmd_status() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let root =
        discover::find_qs_root(&cwd).context("Not in a qs repository. Run 'qs init' first.")?;

    let config = Config::load(&root)?;
    let file_index = qs_core::index::FileIndex::load(&root)?;

    println!("qs repository: {}", root.display());
    println!();
    println!("Configuration:");
    println!("  Model: {}", config.model);
    println!("  Dimension: {}", config.dimension);
    println!("  Chunk size: {} chars", config.chunk_size);
    println!("  Max file size: {} bytes", config.max_file_size);
    println!();
    println!("Index:");
    println!("  Files indexed: {}", file_index.files.len());
    println!(
        "  Total chunks: {}",
        file_index.files.values().map(|f| f.chunk_count).sum::<usize>()
    );

    Ok(())
}

fn cmd_update() -> Result<()> {
    // Update is the same as index - it will skip unchanged files
    cmd_index(None)
}

fn cmd_similar(file: PathBuf, limit: usize) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let root =
        discover::find_qs_root(&cwd).context("Not in a qs repository. Run 'qs init' first.")?;

    let searcher = Searcher::new(root.clone())?;
    let results = searcher.similar(&file, limit)?;

    if results.is_empty() {
        println!("No similar files found.");
        return Ok(());
    }

    let highlighter = SyntaxHighlighter::new();

    println!("Files similar to {}:\n", file.display());

    for (i, result) in results.iter().enumerate() {
        print_result(i + 1, result, &root, &highlighter, 2)?;
    }

    Ok(())
}

fn cmd_search(query: &str, limit: usize, context_lines: usize) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let root =
        discover::find_qs_root(&cwd).context("Not in a qs repository. Run 'qs init' first.")?;

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    pb.set_message("Searching...");

    let searcher = Searcher::new(root.clone())?;
    let results = searcher.search(query, limit)?;

    pb.finish_and_clear();

    if results.is_empty() {
        println!("No results found for: {}", query);
        return Ok(());
    }

    let highlighter = SyntaxHighlighter::new();

    println!("Results for: {}\n", query);

    for (i, result) in results.iter().enumerate() {
        print_result(i + 1, result, &root, &highlighter, context_lines)?;
    }

    Ok(())
}

/// Pretty-print a search result with syntax highlighting.
fn print_result(
    index: usize,
    result: &qs_core::storage::SearchResult,
    root: &Path,
    highlighter: &SyntaxHighlighter,
    context_lines: usize,
) -> Result<()> {
    let score_color = if result.score > 0.7 {
        "\x1b[32m" // Green for high scores
    } else if result.score > 0.5 {
        "\x1b[33m" // Yellow for medium scores
    } else {
        "\x1b[31m" // Red for low scores
    };

    // Header: index, score, file path, line range
    println!(
        "\x1b[1;36m[{}]\x1b[0m {}{:.3}\x1b[0m  \x1b[1m{}\x1b[0m:\x1b[33m{}-{}\x1b[0m",
        index,
        score_color,
        result.score,
        result.payload.path,
        result.payload.start_line,
        result.payload.end_line,
    );

    // Max lines to display before truncating
    const MAX_DISPLAY_LINES: usize = 12;
    const HEAD_LINES: usize = 5;
    const TAIL_LINES: usize = 3;

    // Get the code with context
    let full_path = root.join(&result.payload.path);
    let code_to_display = if full_path.exists() {
        // Try to read the file and get context lines
        if let Ok(content) = std::fs::read_to_string(&full_path) {
            let lines: Vec<&str> = content.lines().collect();
            let start = result.payload.start_line.saturating_sub(context_lines + 1);
            let end = (result.payload.end_line + context_lines).min(lines.len());
            let total_lines = end - start;

            // Build the display text with line numbers, truncating if needed
            let mut display = String::new();

            if total_lines <= MAX_DISPLAY_LINES {
                // Show all lines
                for (i, line) in lines[start..end].iter().enumerate() {
                    let line_num = start + i + 1;
                    let is_match_line = line_num >= result.payload.start_line
                        && line_num <= result.payload.end_line;
                    let prefix = if is_match_line { "│" } else { "┊" };
                    display.push_str(&format!("{} {:4} │ {}\n", prefix, line_num, line));
                }
            } else {
                // Truncate: show first HEAD_LINES, ellipsis, last TAIL_LINES
                let head_end = start + HEAD_LINES;
                let tail_start = end - TAIL_LINES;
                let hidden_lines = total_lines - HEAD_LINES - TAIL_LINES;

                // Head lines
                for (i, line) in lines[start..head_end].iter().enumerate() {
                    let line_num = start + i + 1;
                    let is_match_line = line_num >= result.payload.start_line
                        && line_num <= result.payload.end_line;
                    let prefix = if is_match_line { "│" } else { "┊" };
                    display.push_str(&format!("{} {:4} │ {}\n", prefix, line_num, line));
                }

                // Ellipsis
                display.push_str(&format!(
                    "\x1b[2m     ┊  ... {} more lines ...\x1b[0m\n",
                    hidden_lines
                ));

                // Tail lines
                for (i, line) in lines[tail_start..end].iter().enumerate() {
                    let line_num = tail_start + i + 1;
                    let is_match_line = line_num >= result.payload.start_line
                        && line_num <= result.payload.end_line;
                    let prefix = if is_match_line { "│" } else { "┊" };
                    display.push_str(&format!("{} {:4} │ {}\n", prefix, line_num, line));
                }
            }
            display
        } else {
            // Fall back to stored text
            format_stored_text(&result.payload.text, result.payload.start_line)
        }
    } else {
        // File doesn't exist, use stored text
        format_stored_text(&result.payload.text, result.payload.start_line)
    };

    // Syntax highlight and print
    let extension = Path::new(&result.payload.path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("txt");

    let highlighted = highlighter.highlight(&code_to_display, extension);
    println!("{}", highlighted);
    println!();

    Ok(())
}

fn format_stored_text(text: &str, start_line: usize) -> String {
    const MAX_DISPLAY_LINES: usize = 12;
    const HEAD_LINES: usize = 5;
    const TAIL_LINES: usize = 3;

    let lines: Vec<&str> = text.lines().collect();
    let total_lines = lines.len();
    let mut result = String::new();

    if total_lines <= MAX_DISPLAY_LINES {
        for (i, line) in lines.iter().enumerate() {
            result.push_str(&format!("│ {:4} │ {}\n", start_line + i, line));
        }
    } else {
        // Head
        for (i, line) in lines[..HEAD_LINES].iter().enumerate() {
            result.push_str(&format!("│ {:4} │ {}\n", start_line + i, line));
        }
        // Ellipsis
        let hidden = total_lines - HEAD_LINES - TAIL_LINES;
        result.push_str(&format!(
            "\x1b[2m     ┊  ... {} more lines ...\x1b[0m\n",
            hidden
        ));
        // Tail
        for (i, line) in lines[total_lines - TAIL_LINES..].iter().enumerate() {
            let line_num = start_line + total_lines - TAIL_LINES + i;
            result.push_str(&format!("│ {:4} │ {}\n", line_num, line));
        }
    }
    result
}

/// Wrapper around syntect for syntax highlighting.
struct SyntaxHighlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

impl SyntaxHighlighter {
    fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }

    fn highlight(&self, code: &str, extension: &str) -> String {
        let syntax = self
            .syntax_set
            .find_syntax_by_extension(extension)
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let theme = &self.theme_set.themes["base16-ocean.dark"];
        let mut highlighter = HighlightLines::new(syntax, theme);

        let mut output = String::new();
        for line in LinesWithEndings::from(code) {
            match highlighter.highlight_line(line, &self.syntax_set) {
                Ok(ranges) => {
                    output.push_str(&as_24_bit_terminal_escaped(&ranges[..], false));
                }
                Err(_) => {
                    output.push_str(line);
                }
            }
        }
        output.push_str("\x1b[0m"); // Reset colors

        output
    }
}