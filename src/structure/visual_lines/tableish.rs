use super::VisualLine;

/// Determine if a set of visual lines looks like a true table.
///
/// A table requires:
/// - At least `min_rows` consecutive lines with the same number of spans (≥ 2)
/// - Spans across those lines have consistent X alignment (within tolerance)
/// - Fill rate ≥ 50% (most cells occupied)
///
/// This is much more conservative than the current stream-mode detector:
/// it won't trigger on CV layouts where only some lines have dates on
/// the right.
pub fn looks_like_table(lines: &[VisualLine], min_rows: usize, x_tolerance: f32) -> bool {
    if lines.len() < min_rows {
        return false;
    }

    let span_counts: Vec<usize> = lines.iter().map(|l| l.spans.len()).collect();

    let mut best_run = 0;
    let mut current_run = 1;

    for i in 1..span_counts.len() {
        if span_counts[i] == span_counts[i - 1] && span_counts[i] >= 2 {
            current_run += 1;
            best_run = best_run.max(current_run);
        } else {
            current_run = 1;
        }
    }
    if span_counts.first().is_some_and(|&c| c >= 2) {
        best_run = best_run.max(current_run);
    }

    if best_run < min_rows {
        return false;
    }

    let target_count = span_counts
        .iter()
        .filter(|&&c| c >= 2)
        .copied()
        .max()
        .unwrap_or(0);

    let table_lines: Vec<&VisualLine> = lines
        .iter()
        .filter(|l| l.spans.len() == target_count)
        .collect();

    if table_lines.len() < min_rows {
        return false;
    }

    let first = &table_lines[0];
    let mut aligned_count = 0;

    for line in &table_lines[1..] {
        let all_aligned = first
            .spans
            .iter()
            .zip(line.spans.iter())
            .all(|(a, b)| (a.bbox.x - b.bbox.x).abs() <= x_tolerance);
        if all_aligned {
            aligned_count += 1;
        }
    }

    aligned_count as f32 / (table_lines.len() - 1) as f32 >= 0.6
}
