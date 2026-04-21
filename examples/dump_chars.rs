//! Debug helper: dump raw PDF chars with geometry for a specific page.
//!
//! Usage: cargo run --release --example dump_chars -- <pdf> <page_1based> [substr]
//! Purpose: inspect how inter-word spaces are encoded in the PDF so we can
//! diagnose whether double-spaces come from duplicate real-space glyphs,
//! large TJ displacements, or other encoding choices.

use pdf_oxide::PdfDocument;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args
        .get(1)
        .expect("usage: dump_chars <pdf> <page_1based> [substr]");
    let page_1based: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);
    let sub = args.get(3).map(|s| s.as_str()).unwrap_or("");

    let bytes = std::fs::read(path).expect("read pdf");
    let mut doc = PdfDocument::from_bytes(bytes).expect("parse pdf");
    let chars = doc.extract_chars(page_1based - 1).expect("extract_chars");

    // Cluster into roughly-same-baseline lines purely for visual output.
    let mut line_idx: Option<u32> = None;
    let mut prev_y: f32 = f32::INFINITY;
    let mut buffered_line = String::new();
    let mut line_no = 0u32;

    for ch in &chars {
        let y = ch.origin_y;
        if (prev_y - y).abs() > 3.0 {
            if let Some(ln) = line_idx
                && !buffered_line.is_empty()
                && (sub.is_empty() || buffered_line.contains(sub))
            {
                println!("--- line {} ---\n{}", ln, buffered_line);
                // And dump the raw chars for this line again with geometry.
                let mut line_min_y = f32::INFINITY;
                let mut line_max_y = f32::NEG_INFINITY;
                let mut sampled = Vec::new();
                for c in chars.iter() {
                    let cy = c.origin_y;
                    if (cy - prev_y).abs() < 3.0 && sampled.len() < 300 {
                        line_min_y = line_min_y.min(cy);
                        line_max_y = line_max_y.max(cy);
                        sampled.push(c);
                    }
                }
                // Only dump if line contained substr
                for c in sampled.iter().take(300) {
                    println!(
                        "  ln{} ch={:?} x={:.2} y={:.2} w={:.2} adv={:.2} ox={:.2} font={:?} sz={:.2}",
                        ln,
                        c.char,
                        c.bbox.x,
                        c.origin_y,
                        c.bbox.width,
                        c.advance_width,
                        c.origin_x,
                        c.font_name,
                        c.font_size
                    );
                }
            }
            buffered_line.clear();
            line_no += 1;
            line_idx = Some(line_no);
        }
        buffered_line.push(ch.char);
        prev_y = y;
    }

    if !buffered_line.is_empty() && (sub.is_empty() || buffered_line.contains(sub)) {
        println!("--- line {} (final) ---\n{}", line_no, buffered_line);
    }
}
