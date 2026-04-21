//! CLI argument parsing via clap.
//!
//! Defines the command-line interface for the `olga` binary:
//!
//! ```text
//! olga <INPUT> [OPTIONS]                 — Process a document
//! olga inspect <INPUT>                   — Enriched metadata + health report
//! olga search  <INPUT> <QUERY>           — Case-insensitive text search
//! olga pages   <INPUT> [--page N]        — Page summary (or single-page text)
//! ```

use std::io::IsTerminal;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

use olga::output::OutputFormat;

/// Olga — Intelligent Document Processing
///
/// Extract structured content from PDF, DOCX, XLSX, and HTML documents.
/// Outputs hierarchical JSON with bounding boxes, confidence scores,
/// and provenance tracking — or plain text with faithful spatial placement.
#[derive(Parser, Debug)]
#[command(
    name = "olga",
    version,
    about = "Document extraction for PDF, DOCX, XLSX, and HTML — structured JSON and Markdown with element-level provenance",
    long_about = None,
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Input document path (PDF, DOCX, XLSX, HTML).
    #[arg(value_name = "INPUT", global = true)]
    pub input: Option<PathBuf>,

    /// Output format: json (default), text/txt, markdown/md.
    #[arg(short, long, default_value = "json", global = true)]
    pub format: String,

    /// Write output to file instead of stdout.
    #[arg(short, long, value_name = "FILE", global = true)]
    pub output: Option<PathBuf>,

    /// Pretty-print JSON output (default when writing to a terminal).
    #[arg(long, global = true)]
    pub pretty: bool,

    /// Compact JSON output (no indentation). Overrides --pretty.
    #[arg(long, global = true)]
    pub compact: bool,

    /// Suppress diagnostic messages on stderr.
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Show detailed processing information.
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Disable colored output.
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Print per-step profiling breakdown to stderr.
    #[arg(long, global = true)]
    pub profile: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Show document metadata, a health report, and content counts.
    Inspect {
        /// Input document path.
        #[arg(value_name = "INPUT")]
        input: PathBuf,
    },

    /// Case-insensitive substring search across the document's rendered text.
    Search {
        /// Input document path.
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// Query string to search for.
        #[arg(value_name = "QUERY")]
        query: String,

        /// Restrict the search to a single 1-based page number.
        #[arg(long, value_name = "N")]
        page: Option<usize>,

        /// Cap the number of hits returned.
        #[arg(long, value_name = "N")]
        limit: Option<usize>,
    },

    /// Per-page summary, or single-page text/markdown extraction with `--page`.
    Pages {
        /// Input document path.
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// 1-based page number to extract. When set, prints the rendered page
        /// text (or markdown if `--format markdown`) instead of the summary.
        #[arg(long, value_name = "N")]
        page: Option<usize>,
    },
}

impl Cli {
    /// Resolve the input path — either from the subcommand or the top-level arg.
    pub fn input_path(&self) -> Option<&PathBuf> {
        match &self.command {
            Some(Commands::Inspect { input }) => Some(input),
            Some(Commands::Search { input, .. }) => Some(input),
            Some(Commands::Pages { input, .. }) => Some(input),
            None => self.input.as_ref(),
        }
    }

    /// Resolve the output format.
    pub fn output_format(&self) -> Result<OutputFormat, String> {
        self.format.parse()
    }

    /// Whether to pretty-print JSON.
    ///
    /// Default: pretty when writing to a file or terminal, compact when piping.
    pub fn should_pretty_print(&self) -> bool {
        if self.compact {
            return false;
        }
        if self.pretty {
            return true;
        }
        // Default: pretty for files and terminals, compact only for pipes.
        self.output.is_some() || std::io::stdout().is_terminal()
    }
}
