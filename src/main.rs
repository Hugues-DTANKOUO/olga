// SPDX-License-Identifier: Apache-2.0

//! Olga CLI — the command-line interface for document processing.
//!
//! Wires together format detection, decoding, structure extraction, and output
//! rendering into a single `olga` command.

mod cli;
mod commands;

use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::Instant;

use clap::Parser;

use olga::error::IdpError;
use olga::formats::docx::DocxDecoder;
use olga::formats::html::HtmlDecoder;
use olga::formats::pdf::PdfDecoder;
use olga::formats::xlsx::XlsxDecoder;
use olga::output::OutputFormat;
use olga::structure::{StructureConfig, StructureEngine};
use olga::traits::FormatDecoder;

use cli::{Cli, Commands};

fn main() {
    let cli = Cli::parse();

    let result = match &cli.command {
        Some(Commands::Inspect { input }) => run_inspect(input, &cli),
        Some(Commands::Search {
            input,
            query,
            page,
            limit,
        }) => run_search(input, query, *page, *limit, &cli),
        Some(Commands::Pages { input, page }) => run_pages(input, *page, &cli),
        None => run_process(&cli),
    };

    if let Err(e) = result {
        eprintln!("\x1b[31merror:\x1b[0m {}", e);
        std::process::exit(1);
    }
}

// ---------------------------------------------------------------------------
// Process command (default)
// ---------------------------------------------------------------------------

fn run_process(cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let input = cli
        .input_path()
        .ok_or("no input file specified (usage: olga <file>)")?;

    let format = cli.output_format()?;

    // 1. Read the file.
    let t_start = Instant::now();
    let data = fs::read(input).map_err(|e| format!("cannot read '{}': {}", input.display(), e))?;

    // 2. Detect format and select decoder.
    let ext = input
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let is_pdf = ext == "pdf";
    let is_docx_xlsx = matches!(ext.as_str(), "docx" | "docm" | "xlsx" | "xls");

    // Route logic:
    // - PDF + text/md   → spatial.rs / markdown.rs (char-level, no decode)
    // - DOCX/XLSX + text/md → prim_spatial.rs (decode → primitives, no structure)
    // - HTML + text/md   → text_tree / md_tree (full pipeline)
    // - Any + json       → full pipeline (decode → structure → json)
    // --- Branch 1: PDF + Text/Markdown bypass decode+structure entirely ---
    //
    // The spatial renderers use pdf_oxide character-level bounding boxes
    // directly and only work for PDF.  Non-PDF formats fall through.
    if is_pdf && !matches!(format, OutputFormat::Json) {
        let t_render = Instant::now();
        let (output_string, page_count) = match format {
            OutputFormat::Text => {
                let pages = olga::output::spatial::render_from_bytes(
                    &data,
                    &olga::output::spatial::SpatialConfig {
                        profile: cli.profile,
                        ..Default::default()
                    },
                );
                let pc = pages.last().map_or(0, |p| p.page_number + 1);
                let mut out = String::new();
                for (i, page) in pages.iter().enumerate() {
                    if i > 0 {
                        out.push_str("\n--- page ");
                        out.push_str(&(page.page_number + 1).to_string());
                        out.push_str(" ---\n\n");
                    }
                    for line in &page.lines {
                        out.push_str(line);
                        out.push('\n');
                    }
                }
                (out, pc)
            }
            OutputFormat::Markdown => {
                let pages = olga::output::markdown::render_from_bytes(
                    &data,
                    &olga::output::markdown::MarkdownConfig {
                        profile: cli.profile,
                        ..Default::default()
                    },
                );
                let pc = pages.last().map_or(0, |p| p.page_number + 1);
                let mut out = String::new();
                for (i, page) in pages.iter().enumerate() {
                    if i > 0 {
                        out.push_str("\n---\n\n");
                    }
                    for line in &page.lines {
                        out.push_str(line);
                        out.push('\n');
                    }
                }
                (out, pc)
            }
            // Guarded by `if !needs_structure` above: Json always requires
            // the structure pipeline and never enters this branch.
            OutputFormat::Json => unreachable!("Json format is handled in the structure branch"),
        };
        let render_ms = t_render.elapsed().as_millis();

        if !cli.quiet {
            let size_display = format_size(data.len() as u64);
            eprintln!(
                "  \x1b[36m\u{250c}\x1b[0m \x1b[1molga\x1b[0m v{}",
                env!("CARGO_PKG_VERSION")
            );
            eprintln!(
                "  \x1b[36m\u{2502}\x1b[0m {} \u{00b7} {} \u{00b7} {} pages \u{00b7} {}",
                input.file_name().unwrap_or_default().to_string_lossy(),
                ext.to_uppercase(),
                page_count,
                size_display,
            );
            eprintln!("  \x1b[36m\u{2502}\x1b[0m");
            eprintln!(
                "  \x1b[36m\u{251c}\u{2500}\x1b[0m Rendering ({}) \u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7} {}ms",
                format, render_ms,
            );
        }

        return write_output(cli, &output_string, &t_start, None);
    }

    // --- Branch 2: DOCX/XLSX + Text/Markdown — decode to primitives, then spatial render ---
    //
    // Richer than PDF (semantic hints from the format), but we use the same
    // spatial placement idea: bounding boxes → character grid.
    if is_docx_xlsx && !matches!(format, OutputFormat::Json) {
        let decoder: Box<dyn FormatDecoder> = select_decoder(&ext)?;

        let t_decode = Instant::now();
        let decode_result = decoder.decode(data)?;
        let decode_ms = t_decode.elapsed().as_millis();

        let meta = &decode_result.metadata;
        if !cli.quiet {
            let size_display = format_size(meta.file_size);
            eprintln!(
                "  \x1b[36m\u{250c}\x1b[0m \x1b[1molga\x1b[0m v{}",
                env!("CARGO_PKG_VERSION")
            );
            eprintln!(
                "  \x1b[36m\u{2502}\x1b[0m {} \u{00b7} {} \u{00b7} {} pages \u{00b7} {}",
                input.file_name().unwrap_or_default().to_string_lossy(),
                meta.format,
                meta.page_count,
                size_display,
            );
            eprintln!("  \x1b[36m\u{2502}\x1b[0m");
            eprintln!(
                "  \x1b[36m\u{251c}\u{2500}\x1b[0m Decoding \u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7} {}ms",
                decode_ms,
            );
        }

        let t_render = Instant::now();
        let output_string = match format {
            OutputFormat::Text => olga::output::prim_spatial::render_text(
                &decode_result.primitives,
                &decode_result.metadata,
            ),
            OutputFormat::Markdown => olga::output::prim_spatial::render_markdown(
                &decode_result.primitives,
                &decode_result.metadata,
            ),
            OutputFormat::Json => unreachable!(),
        };
        let render_ms = t_render.elapsed().as_millis();

        if !cli.quiet {
            eprintln!(
                "  \x1b[36m\u{251c}\u{2500}\x1b[0m Rendering ({}) \u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7} {}ms",
                format, render_ms,
            );
        }

        return write_output(cli, &output_string, &t_start, None);
    }

    // --- Branch 3: Full pipeline (JSON for all formats, or HTML text/md) ---
    let decoder: Box<dyn FormatDecoder> = select_decoder(&ext)?;

    // 3. Decode.
    let t_decode = Instant::now();
    let decode_result = decoder.decode(data)?;
    let decode_ms = t_decode.elapsed().as_millis();

    if !cli.quiet {
        // Use metadata from decode_result (no separate metadata() call).
        let meta = &decode_result.metadata;
        let size_display = format_size(meta.file_size);
        eprintln!(
            "  \x1b[36m\u{250c}\x1b[0m \x1b[1molga\x1b[0m v{}",
            env!("CARGO_PKG_VERSION")
        );
        eprintln!(
            "  \x1b[36m\u{2502}\x1b[0m {} \u{00b7} {} \u{00b7} {} pages \u{00b7} {}",
            input.file_name().unwrap_or_default().to_string_lossy(),
            meta.format,
            meta.page_count,
            size_display,
        );
        eprintln!("  \x1b[36m\u{2502}\x1b[0m");
        eprintln!(
            "  \x1b[36m\u{251c}\u{2500}\x1b[0m Decoding \u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7} {}ms",
            decode_ms,
        );
    }

    // 4. Structure detection.
    let t_structure = Instant::now();
    let engine = StructureEngine::new(StructureConfig::default()).with_default_detectors();
    let structure_result = engine.structure(decode_result);
    let structure_ms = t_structure.elapsed().as_millis();

    if !cli.quiet {
        let stats = count_elements(&structure_result.root);
        eprintln!(
            "  \x1b[36m\u{251c}\u{2500}\x1b[0m Structure detection \u{00b7}\u{00b7}\u{00b7} {}ms",
            structure_ms,
        );
        if stats.tables > 0 {
            eprintln!(
                "  \x1b[36m\u{2502}\x1b[0m  \u{251c} {} tables",
                stats.tables,
            );
        }
        if stats.paragraphs > 0 {
            eprintln!(
                "  \x1b[36m\u{2502}\x1b[0m  \u{251c} {} paragraphs",
                stats.paragraphs,
            );
        }
        if stats.headings > 0 {
            eprintln!(
                "  \x1b[36m\u{2502}\x1b[0m  \u{2514} {} headings",
                stats.headings,
            );
        }
        if stats.lists > 0 {
            eprintln!("  \x1b[36m\u{2502}\x1b[0m  \u{2514} {} lists", stats.lists,);
        }
    }

    // 5. Render output.
    let t_render = Instant::now();
    let output_string = match format {
        OutputFormat::Json => {
            let json = olga::output::json::render(&structure_result);
            if cli.should_pretty_print() {
                serde_json::to_string_pretty(&json)?
            } else {
                serde_json::to_string(&json)?
            }
        }
        OutputFormat::Text => olga::output::text_tree::render(&structure_result),
        OutputFormat::Markdown => olga::output::md_tree::render(&structure_result),
    };
    let render_ms = t_render.elapsed().as_millis();

    if !cli.quiet {
        eprintln!(
            "  \x1b[36m\u{251c}\u{2500}\x1b[0m Rendering ({}) \u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7}\u{00b7} {}ms",
            format, render_ms,
        );
    }

    let element_count = structure_result.root.node_count();
    write_output(cli, &output_string, &t_start, Some(element_count))
}

/// Shared output writing for both branches.
fn write_output(
    cli: &Cli,
    output_string: &str,
    t_start: &Instant,
    element_count: Option<usize>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(output_path) = &cli.output {
        fs::write(output_path, output_string)?;
        if !cli.quiet {
            eprintln!("  \x1b[36m\u{2502}\x1b[0m");
            eprintln!(
                "  \x1b[36m\u{2514}\x1b[0m \x1b[32mDone\x1b[0m \u{00b7} {}ms \u{00b7} {} \u{00b7} {}",
                t_start.elapsed().as_millis(),
                format_size(output_string.len() as u64),
                output_path.display(),
            );
        }
    } else {
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        handle.write_all(output_string.as_bytes())?;
        if !output_string.ends_with('\n') {
            handle.write_all(b"\n")?;
        }

        if !cli.quiet {
            let total = t_start.elapsed().as_millis();
            eprintln!("  \x1b[36m\u{2502}\x1b[0m");
            if let Some(elements) = element_count {
                eprintln!(
                    "  \x1b[36m\u{2514}\x1b[0m \x1b[32mDone\x1b[0m \u{00b7} {}ms \u{00b7} {} elements",
                    total, elements,
                );
            } else {
                eprintln!(
                    "  \x1b[36m\u{2514}\x1b[0m \x1b[32mDone\x1b[0m \u{00b7} {}ms",
                    total,
                );
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Inspect command
// ---------------------------------------------------------------------------

fn run_inspect(input: &Path, cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let json = commands::inspect(input)?;
    println!("{}", json_string(&json, cli)?);
    Ok(())
}

// ---------------------------------------------------------------------------
// Search command
// ---------------------------------------------------------------------------

fn run_search(
    input: &Path,
    query: &str,
    page: Option<usize>,
    limit: Option<usize>,
    cli: &Cli,
) -> Result<(), Box<dyn std::error::Error>> {
    let json = commands::search(input, query, page, limit)?;
    println!("{}", json_string(&json, cli)?);
    Ok(())
}

// ---------------------------------------------------------------------------
// Pages command
// ---------------------------------------------------------------------------

fn run_pages(
    input: &Path,
    page: Option<usize>,
    cli: &Cli,
) -> Result<(), Box<dyn std::error::Error>> {
    // When `--page N` is set we honour `--format` to flip between plain
    // text and markdown. Summary mode is always JSON — mixing a summary
    // with text/markdown would defeat the purpose of a structured listing.
    let fmt = cli.output_format().unwrap_or(OutputFormat::Json);
    let want_markdown = matches!(fmt, OutputFormat::Markdown);

    match commands::pages(input, page, want_markdown)? {
        commands::PagesOutput::Summary(json) => {
            println!("{}", json_string(&json, cli)?);
        }
        commands::PagesOutput::PageText(body) => {
            let stdout = std::io::stdout();
            let mut handle = stdout.lock();
            handle.write_all(body.as_bytes())?;
            if !body.ends_with('\n') {
                handle.write_all(b"\n")?;
            }
        }
    }
    Ok(())
}

/// Pretty- or compact-print a JSON value based on the CLI flags.
fn json_string(value: &serde_json::Value, cli: &Cli) -> Result<String, serde_json::Error> {
    if cli.should_pretty_print() {
        serde_json::to_string_pretty(value)
    } else {
        serde_json::to_string(value)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn select_decoder(ext: &str) -> Result<Box<dyn FormatDecoder>, Box<dyn std::error::Error>> {
    match ext {
        "pdf" => Ok(Box::new(PdfDecoder)),
        "docx" | "docm" => Ok(Box::new(DocxDecoder)),
        "xlsx" | "xls" => Ok(Box::new(XlsxDecoder)),
        "html" | "htm" => Ok(Box::new(HtmlDecoder)),
        other => Err(Box::new(IdpError::UnsupportedFormat(format!(
            "unknown extension '.{}' (supported: pdf, docx, xlsx, html)",
            other
        )))),
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

struct ElementStats {
    tables: u32,
    paragraphs: u32,
    headings: u32,
    lists: u32,
}

fn count_elements(node: &olga::model::DocumentNode) -> ElementStats {
    let mut stats = ElementStats {
        tables: 0,
        paragraphs: 0,
        headings: 0,
        lists: 0,
    };
    count_recursive(node, &mut stats);
    stats
}

fn count_recursive(node: &olga::model::DocumentNode, stats: &mut ElementStats) {
    match &node.kind {
        olga::model::NodeKind::Table { .. } => stats.tables += 1,
        olga::model::NodeKind::Paragraph { .. } => stats.paragraphs += 1,
        olga::model::NodeKind::Heading { .. } => stats.headings += 1,
        olga::model::NodeKind::List { .. } => stats.lists += 1,
        _ => {}
    }
    for child in &node.children {
        count_recursive(child, stats);
    }
}
