//! Spatial renderer — pure geometric text placement from individual characters.
//!
//! Uses pdf_oxide's `extract_chars()` to get character-level primitives with
//! exact bounding boxes. Groups chars into words by physical proximity,
//! words into lines by Y position, then places on a character grid.
//!
//! No semantic detection. Pure geometry from the actual character coordinates.

mod grid;
mod placement;
mod words;

use std::time::Instant;

use grid::render_page;
use placement::build_placed_texts;
use words::group_chars_into_words;

use crate::output::rules::{extract_pdf_segments, normalize_segments, pdf_segments_bounds};

/// Configuration for the spatial renderer.
#[derive(Default)]
pub struct SpatialConfig {
    pub target_cols_override: usize,
    pub profile: bool,
}

/// Profiling breakdown for the spatial renderer.
#[derive(Default)]
pub struct SpatialProfile {
    pub pdf_parse_us: u64,
    pub page_count_us: u64,
    pub extract_chars_us: u64,
    pub annotations_us: u64,
    pub word_grouping_us: u64,
    pub link_matching_us: u64,
    pub coord_normalize_us: u64,
    pub placement_us: u64,
    pub rules_extract_us: u64,
    pub grid_render_us: u64,
    pub total_pages: usize,
}

impl SpatialProfile {
    pub fn print_stderr(&self) {
        let total = self.pdf_parse_us
            + self.page_count_us
            + self.extract_chars_us
            + self.annotations_us
            + self.word_grouping_us
            + self.link_matching_us
            + self.coord_normalize_us
            + self.placement_us
            + self.rules_extract_us
            + self.grid_render_us;
        let ms = |us: u64| us as f64 / 1000.0;
        let pct = |us: u64| {
            if total > 0 {
                (us as f64 / total as f64) * 100.0
            } else {
                0.0
            }
        };

        eprintln!(
            "  \x1b[35m┌─ Profile (spatial)\x1b[0m  {} pages, {:.1}ms total",
            self.total_pages,
            ms(total)
        );
        eprintln!(
            "  \x1b[35m│\x1b[0m  pdf_parse ········· {:>8.1}ms  ({:>5.1}%)",
            ms(self.pdf_parse_us),
            pct(self.pdf_parse_us)
        );
        eprintln!(
            "  \x1b[35m│\x1b[0m  page_count ········ {:>8.1}ms  ({:>5.1}%)",
            ms(self.page_count_us),
            pct(self.page_count_us)
        );
        eprintln!(
            "  \x1b[35m│\x1b[0m  extract_chars ····· {:>8.1}ms  ({:>5.1}%)",
            ms(self.extract_chars_us),
            pct(self.extract_chars_us)
        );
        eprintln!(
            "  \x1b[35m│\x1b[0m  annotations ······· {:>8.1}ms  ({:>5.1}%)",
            ms(self.annotations_us),
            pct(self.annotations_us)
        );
        eprintln!(
            "  \x1b[35m│\x1b[0m  word_grouping ····· {:>8.1}ms  ({:>5.1}%)",
            ms(self.word_grouping_us),
            pct(self.word_grouping_us)
        );
        eprintln!(
            "  \x1b[35m│\x1b[0m  link_matching ····· {:>8.1}ms  ({:>5.1}%)",
            ms(self.link_matching_us),
            pct(self.link_matching_us)
        );
        eprintln!(
            "  \x1b[35m│\x1b[0m  coord_normalize ··· {:>8.1}ms  ({:>5.1}%)",
            ms(self.coord_normalize_us),
            pct(self.coord_normalize_us)
        );
        eprintln!(
            "  \x1b[35m│\x1b[0m  placement ········· {:>8.1}ms  ({:>5.1}%)",
            ms(self.placement_us),
            pct(self.placement_us)
        );
        eprintln!(
            "  \x1b[35m│\x1b[0m  rules_extract ····· {:>8.1}ms  ({:>5.1}%)",
            ms(self.rules_extract_us),
            pct(self.rules_extract_us)
        );
        eprintln!(
            "  \x1b[35m└\x1b[0m  grid_render ······· {:>8.1}ms  ({:>5.1}%)",
            ms(self.grid_render_us),
            pct(self.grid_render_us)
        );
    }
}

pub struct SpatialPage {
    pub page_number: u32,
    pub lines: Vec<String>,
}

pub fn render_from_bytes(pdf_bytes: &[u8], config: &SpatialConfig) -> Vec<SpatialPage> {
    let profile = config.profile;
    let mut prof = SpatialProfile::default();

    let t = Instant::now();
    let mut doc = match pdf_oxide::PdfDocument::from_bytes(pdf_bytes.to_vec()) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    prof.pdf_parse_us = t.elapsed().as_micros() as u64;

    let t = Instant::now();
    let page_count = doc.page_count().unwrap_or(0);
    prof.page_count_us = t.elapsed().as_micros() as u64;
    prof.total_pages = page_count;

    let mut output = Vec::new();

    for page_idx in 0..page_count {
        let t = Instant::now();
        let chars = match doc.extract_chars(page_idx) {
            Ok(c) => c,
            Err(_) => continue,
        };
        prof.extract_chars_us += t.elapsed().as_micros() as u64;

        if chars.is_empty() {
            continue;
        }

        // Step 1: Extract link annotations.
        let t = Instant::now();
        let link_annots: Vec<(f32, f32, f32, f32, String)> = doc
            .get_annotations(page_idx)
            .unwrap_or_default()
            .iter()
            .filter_map(|a| {
                let uri = match &a.action {
                    Some(pdf_oxide::annotations::LinkAction::Uri(u)) => u.clone(),
                    _ => return None,
                };
                let rect = a.rect?;
                Some((
                    rect[0] as f32,
                    rect[1] as f32,
                    rect[2] as f32,
                    rect[3] as f32,
                    uri,
                ))
            })
            .collect();
        prof.annotations_us += t.elapsed().as_micros() as u64;

        // Step 2: Group chars into words.
        let t = Instant::now();
        let mut words = group_chars_into_words(&chars);
        prof.word_grouping_us += t.elapsed().as_micros() as u64;

        // Step 3: Associate words with link annotations.
        let t = Instant::now();
        for word in &mut words {
            let cx = word.x + word.w / 2.0;
            let cy = word.y + word.h / 2.0;
            for (x1, y1, x2, y2, uri) in &link_annots {
                if cx >= *x1 && cx <= *x2 && cy >= *y1 && cy <= *y2 {
                    word.link = Some(uri.clone());
                    break;
                }
            }
        }
        prof.link_matching_us += t.elapsed().as_micros() as u64;

        // Step 4: Extract stroked-line primitives in raw PDF coords so we
        // can expand the normalization bounds to include them. Words and
        // rules must share one coordinate frame, otherwise a rule that
        // extends beyond the text bbox gets silently clipped away.
        let t = Instant::now();
        let pdf_segments = extract_pdf_segments(&mut doc, page_idx);
        prof.rules_extract_us += t.elapsed().as_micros() as u64;

        // Step 5: Normalize coordinates over the union of word + rule bounds.
        let t = Instant::now();
        let mut content_min_x = words.iter().map(|w| w.x).fold(f32::MAX, f32::min);
        let mut content_max_x = words.iter().map(|w| w.x + w.w).fold(f32::MIN, f32::max);
        let mut content_min_y = words.iter().map(|w| w.y).fold(f32::MAX, f32::min);
        let mut content_max_y = words.iter().map(|w| w.y + w.h).fold(f32::MIN, f32::max);

        if let Some((rmin_x, rmin_y, rmax_x, rmax_y)) = pdf_segments_bounds(&pdf_segments) {
            content_min_x = content_min_x.min(rmin_x);
            content_min_y = content_min_y.min(rmin_y);
            content_max_x = content_max_x.max(rmax_x);
            content_max_y = content_max_y.max(rmax_y);
        }

        let cw = (content_max_x - content_min_x).max(1.0);
        let ch = (content_max_y - content_min_y).max(1.0);
        prof.coord_normalize_us += t.elapsed().as_micros() as u64;

        // Step 6: Build placed texts and normalize rule segments in the
        // same coordinate frame.
        let t = Instant::now();
        let placed = build_placed_texts(&words, content_min_x, content_min_y, cw, ch);
        prof.placement_us += t.elapsed().as_micros() as u64;

        let t = Instant::now();
        let raw_segments = normalize_segments(&pdf_segments, content_min_x, content_min_y, cw, ch);
        prof.rules_extract_us += t.elapsed().as_micros() as u64;

        // Step 7: Render on grid. The content aspect ratio (ch/cw) lets the
        // grid calibrate rows against columns so that 1 pt horizontal and 1
        // pt vertical render to visually equivalent extents on a terminal.
        let content_aspect = ch / cw;
        let t = Instant::now();
        let lines = render_page(&placed, &raw_segments, content_aspect, config);
        prof.grid_render_us += t.elapsed().as_micros() as u64;

        output.push(SpatialPage {
            page_number: page_idx as u32,
            lines,
        });
    }

    if profile {
        prof.print_stderr();
    }

    output
}
