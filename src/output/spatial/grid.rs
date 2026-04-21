use std::collections::{BTreeMap, HashMap, HashSet};

use super::SpatialConfig;
use super::placement::{PlacedText, compute_target_cols};
use super::words::merge_cross_font_adjacent_placed;
use crate::output::col_resolve::resolve_cols;
use crate::output::row_cluster::snap_same_row;
use crate::output::rules::{GridRule, RawSegment, adjust_rules, snap_to_grid};

pub(super) fn render_page(
    placed: &[PlacedText],
    rules: &[RawSegment],
    content_aspect: f32,
    config: &SpatialConfig,
) -> Vec<String> {
    if placed.is_empty() {
        return Vec::new();
    }

    // `target_cols` must be computed from the *unmerged* placements so
    // merging never inflates the `chars/width` ratios used to calibrate
    // the grid. If merging fed back into this p90, documents with mixed
    // regular/bold cells (weird_invoice) would see their pages widen by
    // the synth-space overhead, shifting every pristine line by a
    // column or two.
    let target_cols = if config.target_cols_override > 0 {
        config.target_cols_override
    } else {
        compute_target_cols(placed)
    };

    // Merge cross-font fragments that the font-bucket tokenizer in
    // `group_chars_into_words` separated. See `merge_cross_font_adjacent_placed`
    // — the merge reunites regular/bold word fragments on the same baseline
    // so the grid paints them as a single contiguous block starting at the
    // left fragment's column, eliminating the proportional-width spill
    // visible as double-spaces (Cours CR300 v2 "examen final du cours").
    let merged_placed = merge_cross_font_adjacent_placed(placed.to_vec(), target_cols);
    let placed = merged_placed.as_slice();

    // target_rows sizes the grid to fit the tallest content on the page,
    // which means taking the max Y across BOTH placed words AND rules.
    // When text is narrow but a ruler or table border extends further
    // down, using words alone would clip the rule off the grid.
    let max_y_words = placed.iter().map(|p| p.y).fold(0.0f32, f32::max);
    let max_y_rules = rules.iter().map(|r| r.y0.max(r.y1)).fold(0.0f32, f32::max);
    let max_y = max_y_words.max(max_y_rules).max(0.1);

    // Calibrate so one row covers the same *visual* extent as one column.
    // Terminal characters render ~2:1 (tall:wide), so we want
    // `pt_per_row = 2 * pt_per_col`, i.e. `target_rows = target_cols *
    // (ch/cw) / 2`. Without this calibration, vertical extents are stretched
    // by the char aspect ratio: a 0.85pt mesh-pixel rendered on a grid where
    // rows span 1.64pt ends up bleeding across 12 rows for what geometrically
    // occupies ~2 text lines in PDF. `content_aspect` is `ch / cw` of the
    // shared word+rule bounding box.
    let aspect = content_aspect.max(1e-3);
    let target_rows = ((max_y + 0.05) * target_cols as f32 * aspect * 0.5).round() as usize;
    let target_rows = target_rows.clamp(10, 500);

    let mut grid: Vec<Vec<char>> = vec![vec![' '; target_cols]; target_rows];
    let mut row_links: BTreeMap<usize, Vec<(String, String)>> = BTreeMap::new();

    // Word rows must never be shared with a horizontal rule — a rule
    // bleeding through text turns the line into something like
    // `|Discovery----|----6----|`. For each word-occupied row we record
    // the word's UNROUNDED fractional row; the rule-adjustment step uses
    // it to decide which side of the row the rule geometrically belongs
    // on (strictly from the relative PDF positions, no semantic bias).
    // Vertical rules pass through text rows normally — that's the ASCII
    // table convention.
    // Linear map `[0, 1] → [0, last_row]`: y=1.0 lands exactly on the last
    // row instead of requiring a clamp from `N` to `N-1`. The clamped
    // mapping used to collapse the bottom two textual rows into one grid
    // row when they were both within half a cell of the boundary — that
    // was the weird_invoice footer fusion (`IBAN CH93 … 2011 6238 …` +
    // `BIC UBSWCHZH80A` rendered on one line with overwrite).
    let last_row = target_rows.saturating_sub(1);
    let rows_f = (last_row as f32).max(1.0);
    // Snap words sharing a PDF baseline to a common exact row BEFORE the
    // integer-row mapping. The previous `ny * target_rows` convention
    // absorbed sub-cell baseline jitter by clamping everything past
    // `target_rows - 0.5` onto the last row — a side effect that glued
    // same-line words together but also fused the footer two-liner on
    // `weird_invoice`. The current `ny * (target_rows - 1)` mapping is
    // geometrically correct but exposes the jitter anywhere on the page
    // (a header field and a contact line on the same baseline can now
    // round to different integer rows if their unrounded positions
    // straddle a cell boundary). Clustering at < 0.5 cell recovers the
    // same-line invariant without reintroducing the bottom-row pile-up
    // — natural line spacing of ≥ 1 full cell always breaks the chain.
    let raw_exacts: Vec<f32> = placed.iter().map(|p| p.y * rows_f).collect();
    let exacts = snap_same_row(&raw_exacts);
    let mut word_exact_rows: HashMap<usize, f32> = HashMap::new();
    for &exact in &exacts {
        let row = (exact.round() as usize).min(last_row);
        // If several words land on the same integer row, keep the one
        // with the largest exact y (the "lowest" geometrically). That
        // biases rule-placement toward putting shifted rules ABOVE the
        // text when in doubt, which matches the reading-order convention
        // for a header row; but the decision is still made purely from
        // numeric y comparison, not from any classification.
        word_exact_rows
            .entry(row)
            .and_modify(|e| {
                if exact > *e {
                    *e = exact;
                }
            })
            .or_insert(exact);
    }

    // Draw rules first so words overwrite on collision (text always wins).
    // Junctions and corners are imperfect by design — last-write-wins on
    // intersecting H/V is the accepted outcome of pure preservation.
    let snapped = snap_to_grid(rules, target_cols, target_rows);
    for rule in adjust_rules(snapped, &word_exact_rows, target_rows) {
        draw_rule(&mut grid, rule);
    }

    let last_col = target_cols.saturating_sub(1);
    let cols_f = (last_col as f32).max(1.0);

    // Resolve same-row column collisions BEFORE writing to the grid.
    // Without this pass, two words whose intended columns overlap (or
    // round to touching intervals) overwrite each other under the
    // writer's last-write-wins rule — see `col_resolve` for the three
    // real-document defects this fixes. Rows and intended cols are
    // computed once here so both the resolver and the writer share
    // identical assignments.
    let rows_int: Vec<usize> = exacts
        .iter()
        .map(|&e| (e.round() as usize).min(last_row))
        .collect();
    let intended_cols: Vec<usize> = placed
        .iter()
        .map(|p| ((p.x * cols_f).round() as usize).min(last_col))
        .collect();
    let lens: Vec<usize> = placed.iter().map(|p| p.text.chars().count()).collect();
    let actual_cols = resolve_cols(&rows_int, &intended_cols, &lens);

    for (i, p) in placed.iter().enumerate() {
        let row = rows_int[i];
        let col = actual_cols[i];

        for (c, ch) in (col..).zip(p.text.chars()) {
            if c >= grid[row].len() {
                grid[row].resize(c + 1, ' ');
            }
            grid[row][c] = ch;
        }

        if let Some(ref url) = p.link {
            row_links
                .entry(row)
                .or_default()
                .push((p.text.clone(), url.clone()));
        }
    }

    let mut lines: Vec<String> = Vec::new();
    for (row_idx, row) in grid.iter().enumerate() {
        let text: String = row.iter().collect::<String>().trim_end().to_string();
        if text.is_empty() {
            continue;
        }
        lines.push(text);

        if let Some(links) = row_links.get(&row_idx) {
            let mut seen = HashSet::new();
            let urls: Vec<&str> = links
                .iter()
                .filter_map(|(_, url)| {
                    if seen.insert(url.as_str()) {
                        Some(url.as_str())
                    } else {
                        None
                    }
                })
                .collect();
            if !urls.is_empty()
                && let Some(last) = lines.last_mut()
            {
                for url in urls {
                    last.push_str("  <");
                    last.push_str(url);
                    last.push('>');
                }
            }
        }
    }

    lines
}

fn draw_rule(grid: &mut [Vec<char>], rule: GridRule) {
    match rule {
        GridRule::Horizontal {
            row,
            col_start,
            col_end,
            ..
        } => {
            if row >= grid.len() {
                return;
            }
            let line = &mut grid[row];
            if col_end >= line.len() {
                line.resize(col_end + 1, ' ');
            }
            for cell in &mut line[col_start..=col_end] {
                *cell = '-';
            }
        }
        GridRule::Vertical {
            col,
            row_start,
            row_end,
            ..
        } => {
            let last = row_end.min(grid.len().saturating_sub(1));
            if row_start > last {
                return;
            }
            for line in &mut grid[row_start..=last] {
                if col >= line.len() {
                    line.resize(col + 1, ' ');
                }
                line[col] = '|';
            }
        }
    }
}
