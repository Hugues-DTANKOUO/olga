use std::collections::HashMap;

use super::MarkdownConfig;
use super::placement::{PlacedWord, compute_target_cols};
use super::words::merge_cross_font_adjacent_placed;
use crate::output::col_resolve::resolve_cols;
use crate::output::row_cluster::snap_same_row;
use crate::output::rules::{GridRule, RawSegment, adjust_rules, snap_to_grid};

struct GridCell {
    ch: char,
}

#[derive(Clone)]
struct FormattedRun {
    col_start: usize,
    col_end: usize,
    is_bold: bool,
    is_italic: bool,
    link: Option<String>,
}

pub(super) fn render_page(
    placed: &[PlacedWord],
    rules: &[RawSegment],
    content_aspect: f32,
    config: &MarkdownConfig,
) -> Vec<String> {
    if placed.is_empty() {
        return Vec::new();
    }

    // `target_cols` must be computed from the *unmerged* placements so
    // merging never inflates the `chars/width` ratios used to calibrate
    // the grid. See `merge_cross_font_adjacent_placed` for why.
    let target_cols = if config.target_cols_override > 0 {
        config.target_cols_override
    } else {
        compute_target_cols(placed)
    };

    // Merge cross-font fragments that the font-bucket tokenizer split
    // apart — reunites regular/bold fragments on the same baseline so
    // the grid paints them contiguously instead of leaking proportional
    // width as double-spaces.
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

    let mut grid: Vec<Vec<GridCell>> = (0..target_rows)
        .map(|_| (0..target_cols).map(|_| GridCell { ch: ' ' }).collect())
        .collect();
    let mut row_runs: Vec<Vec<FormattedRun>> = vec![Vec::new(); target_rows];

    // Word rows must stay distinct from horizontal-rule rows; otherwise
    // the reader sees `|Discovery----|----6----|` lines where text and
    // separator bleed together. For each word-occupied row we record the
    // word's UNROUNDED fractional row; the rule-adjustment step uses it
    // to pick the nudge direction purely from numeric y comparison (no
    // semantic bias about where a separator "should" go relative to its
    // table header). Vertical rules still flow through text rows — that
    // is the standard ASCII cell-separator style.
    // Linear map `[0, 1] → [0, last_row]`: y=1.0 lands exactly on the last
    // row instead of requiring a clamp from `N` to `N-1`. See the twin
    // comment in `spatial/grid.rs` — the previous `y * target_rows` path
    // collapsed footer rows 3 and 4 onto the same grid row under the
    // clamp.
    let last_row = target_rows.saturating_sub(1);
    let rows_f = (last_row as f32).max(1.0);
    // Snap same-baseline words to a common exact row before rounding — see
    // the twin comment in `spatial/grid.rs` and `row_cluster::snap_same_row`
    // for rationale. Same-baseline words whose unrounded row positions
    // straddle a `k + 0.5` boundary otherwise split onto adjacent rows.
    let raw_exacts: Vec<f32> = placed.iter().map(|p| p.y * rows_f).collect();
    let exacts = snap_same_row(&raw_exacts);
    let mut word_exact_rows: HashMap<usize, f32> = HashMap::new();
    for &exact in &exacts {
        let row = (exact.round() as usize).min(last_row);
        word_exact_rows
            .entry(row)
            .and_modify(|e| {
                if exact > *e {
                    *e = exact;
                }
            })
            .or_insert(exact);
    }

    // Draw rules first so words overwrite on collision. Rules never carry
    // markdown formatting (no run is registered), so `-` and `|` emerge as
    // plain ASCII in the final line regardless of surrounding style runs.
    let snapped = snap_to_grid(rules, target_cols, target_rows);
    for rule in adjust_rules(snapped, &word_exact_rows, target_rows) {
        draw_rule(&mut grid, rule);
    }

    let last_col = target_cols.saturating_sub(1);
    let cols_f = (last_col as f32).max(1.0);

    // Same collision-resolution pass as the spatial renderer — see
    // `col_resolve` for the full rationale. Formatted runs must be
    // registered at the RESOLVED column, not the intended one, otherwise
    // markdown markers wrap empty space or miss the word they decorate.
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

        if p.is_bold || p.is_italic || p.link.is_some() {
            let text_len = lens[i];
            row_runs[row].push(FormattedRun {
                col_start: col,
                col_end: col + text_len,
                is_bold: p.is_bold,
                is_italic: p.is_italic,
                link: p.link.clone(),
            });
        }

        for (c, ch) in (col..).zip(p.text.chars()) {
            if c >= grid[row].len() {
                grid[row].resize_with(c + 1, || GridCell { ch: ' ' });
            }
            grid[row][c] = GridCell { ch };
        }
    }

    let mut lines: Vec<String> = Vec::new();
    for (row_idx, row) in grid.iter().enumerate() {
        let raw_text: String = row.iter().map(|cell| cell.ch).collect::<String>();
        let trimmed = raw_text.trim_end();
        if trimmed.is_empty() {
            continue;
        }

        let runs = &row_runs[row_idx];
        if runs.is_empty() {
            lines.push(trimmed.to_string());
            continue;
        }

        let merged = merge_adjacent_runs(runs);
        let line = render_line_with_markdown(row, trimmed.len(), &merged);
        lines.push(line);
    }

    lines
}

fn draw_rule(grid: &mut [Vec<GridCell>], rule: GridRule) {
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
                line.resize_with(col_end + 1, || GridCell { ch: ' ' });
            }
            for cell in &mut line[col_start..=col_end] {
                cell.ch = '-';
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
                    line.resize_with(col + 1, || GridCell { ch: ' ' });
                }
                line[col].ch = '|';
            }
        }
    }
}

fn merge_adjacent_runs(runs: &[FormattedRun]) -> Vec<FormattedRun> {
    if runs.is_empty() {
        return Vec::new();
    }

    let mut sorted: Vec<&FormattedRun> = runs.iter().collect();
    sorted.sort_by_key(|r| r.col_start);

    let mut merged: Vec<FormattedRun> = Vec::new();
    for run in sorted {
        let should_extend = merged.last().is_some_and(|last| {
            last.is_bold == run.is_bold
                && last.is_italic == run.is_italic
                && last.link == run.link
                && run.col_start <= last.col_end + 2
        });

        if should_extend {
            if let Some(last) = merged.last_mut() {
                last.col_end = last.col_end.max(run.col_end);
            }
        } else {
            merged.push(FormattedRun {
                col_start: run.col_start,
                col_end: run.col_end,
                is_bold: run.is_bold,
                is_italic: run.is_italic,
                link: run.link.clone(),
            });
        }
    }

    merged
}

fn render_line_with_markdown(
    row: &[GridCell],
    trimmed_len: usize,
    runs: &[FormattedRun],
) -> String {
    let chars: Vec<char> = row.iter().take(trimmed_len).map(|c| c.ch).collect();

    let mut events: Vec<(usize, bool, String)> = Vec::new();
    for run in runs {
        let start = run.col_start.min(trimmed_len);
        let end = run.col_end.min(trimmed_len);
        if start >= end {
            continue;
        }

        let mut open = String::new();
        if run.is_bold {
            open.push_str("**");
        }
        if run.is_italic {
            open.push('*');
        }
        if run.link.is_some() {
            open.push('[');
        }

        let mut close = String::new();
        if run.link.is_some() {
            close.push(']');
            close.push('(');
            close.push_str(run.link.as_deref().unwrap_or(""));
            close.push(')');
        }
        if run.is_italic {
            close.push('*');
        }
        if run.is_bold {
            close.push_str("**");
        }

        events.push((start, true, open));
        events.push((end, false, close));
    }

    // Close events fire before open events at the same column so that
    // back-to-back runs (`**a****b**`) close `a` before opening `b`.
    events.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    // Markdown markers (`**`, `*`, `[`, `](url)`) inflate the output string
    // without reserving grid cells. Left unchecked, every emission pushes
    // later text — including `|` table separators — further right, so a
    // bold header row no longer lines up with the plain rows below it.
    //
    // We compensate by treating the INTER-run whitespace as the budget
    // for marker overhead:
    //
    //   - An OPEN marker first backfills into trailing spaces already
    //     emitted to `result` (so the run's visible text can start
    //     exactly at its grid column). Any unreclaimed length is carried
    //     forward as `skip_spaces`, to be absorbed by following spaces
    //     AFTER the run closes.
    //   - A CLOSE marker adds its own length to `skip_spaces`, then the
    //     outer loop consumes subsequent space cells until the debt
    //     clears.
    //
    // Critically, `skip_spaces` is ONLY allowed to consume whitespace
    // that sits OUTSIDE any active run. Spaces between words inside a
    // bold run (`**Quarterly Engineering Report**`) are content, not
    // padding — swallowing them would corrupt the text itself. The
    // `run_depth` counter tracks how many runs are currently open, so
    // consumption only happens when `run_depth == 0`. If a run's drift
    // can't be reclaimed before the next non-space sentinel arrives,
    // it becomes permanent rightward drift — which is intrinsic to
    // markers like `**` that have no zero-width representation.
    let mut result = String::with_capacity(trimmed_len + events.len() * 6);
    let mut skip_spaces: usize = 0;
    let mut run_depth: usize = 0;
    let mut event_idx = 0;

    for (col, &ch) in chars.iter().enumerate() {
        while event_idx < events.len() && events[event_idx].0 == col {
            let (_, is_open, marker) = &events[event_idx];
            let marker_len = marker.chars().count();
            if *is_open {
                // Reclaim as many trailing spaces as possible so the
                // opening marker sits in the gap before the run rather
                // than pushing everything right.
                let mut reclaimed = 0usize;
                while reclaimed < marker_len && result.ends_with(' ') {
                    result.pop();
                    reclaimed += 1;
                }
                skip_spaces += marker_len - reclaimed;
                result.push_str(marker);
                run_depth += 1;
            } else {
                result.push_str(marker);
                skip_spaces += marker_len;
                run_depth = run_depth.saturating_sub(1);
            }
            event_idx += 1;
        }

        if run_depth == 0 && skip_spaces > 0 {
            if ch == ' ' {
                // Outside any run, a trailing space pays down marker debt.
                skip_spaces -= 1;
                continue;
            }
            // Outside a run but non-space content arrives — debt becomes
            // permanent drift (markers like `**` have no zero-width form).
            skip_spaces = 0;
        }
        result.push(ch);
    }

    // Flush any trailing close events that fire past the last cell — they
    // can't consume spaces (there are none) but must still be written so
    // the run is well-formed.
    while event_idx < events.len() {
        result.push_str(&events[event_idx].2);
        event_idx += 1;
    }

    result.trim_end().to_string()
}
