//! Integration tests for the rules preservation in the text / markdown
//! spatial renderers.
//!
//! We build minimal valid PDFs inline (mirroring the approach in
//! `pdf_tests.rs`) that combine text with stroked lines or rectangles,
//! then render them through `output::spatial::render_from_bytes` and
//! `output::markdown::render_from_bytes` and assert that the resulting
//! character grid contains the expected `-` / `|` glyphs in roughly the
//! right places.
//!
//! Assertions deliberately check for *presence* of the rule glyphs rather
//! than exact positions, because the spatial renderer auto-scales columns
//! / rows from content density. Position-checking would make the tests
//! brittle to harmless changes in `compute_target_cols`.

use olga::output::markdown::{self, MarkdownConfig};
use olga::output::spatial::{self, SpatialConfig};

// ---------------------------------------------------------------------------
// PDF builder helpers
// ---------------------------------------------------------------------------

/// Build a minimal valid PDF with a single page whose content stream is
/// exactly `content_stream`. The page uses Type1 Helvetica under /F1 so
/// `BT ... Tj ET` blocks work, and also declares an empty ExtGState so
/// `m l S`-style stroke operators are legal.
///
/// MediaBox is 612 × 792 (US Letter). PDF coordinates are bottom-up:
/// y = 792 is the top of the page.
fn build_pdf(content_stream: &str) -> Vec<u8> {
    let len = content_stream.len();

    let mut pdf = String::new();
    pdf.push_str("%PDF-1.4\n");

    let obj1 = pdf.len();
    pdf.push_str("1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
    let obj2 = pdf.len();
    pdf.push_str("2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");
    let obj3 = pdf.len();
    pdf.push_str(
        "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
         /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n",
    );
    let obj4 = pdf.len();
    pdf.push_str(&format!(
        "4 0 obj\n<< /Length {} >>\nstream\n{}\nendstream\nendobj\n",
        len, content_stream
    ));
    let obj5 = pdf.len();
    pdf.push_str("5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n");

    let xref = pdf.len();
    pdf.push_str("xref\n0 6\n");
    pdf.push_str("0000000000 65535 f \n");
    pdf.push_str(&format!("{:010} 00000 n \n", obj1));
    pdf.push_str(&format!("{:010} 00000 n \n", obj2));
    pdf.push_str(&format!("{:010} 00000 n \n", obj3));
    pdf.push_str(&format!("{:010} 00000 n \n", obj4));
    pdf.push_str(&format!("{:010} 00000 n \n", obj5));

    pdf.push_str(&format!(
        "trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
        xref
    ));

    pdf.into_bytes()
}

/// A page with a piece of text and one horizontal stroked line that spans
/// the content width below the text. This simulates an underline / ruler.
fn horizontal_line_pdf() -> Vec<u8> {
    let content = "\
q 1 w \
72 600 m 540 600 l S \
Q \
BT /F1 14 Tf 72 700 Td (Header line) Tj ET";
    build_pdf(content)
}

/// A page with text on the left and a vertical stroked line to the right.
fn vertical_line_pdf() -> Vec<u8> {
    let content = "\
q 1 w \
400 300 m 400 680 l S \
Q \
BT /F1 12 Tf 72 700 Td (Left column text) Tj ET \
BT /F1 12 Tf 72 650 Td (Second text line) Tj ET \
BT /F1 12 Tf 72 600 Td (Third text line) Tj ET";
    build_pdf(content)
}

/// A page with a stroked rectangle (a "box") around a short label, plus
/// some additional text to anchor the spatial grid.
fn stroked_rect_pdf() -> Vec<u8> {
    let content = "\
q 1 w \
200 500 80 40 re S \
Q \
BT /F1 12 Tf 210 515 Td (BOX) Tj ET \
BT /F1 12 Tf 72 700 Td (Top of page anchor) Tj ET \
BT /F1 12 Tf 72 400 Td (Bottom anchor text) Tj ET";
    build_pdf(content)
}

/// A 3-line grid of horizontal rules crossed by 3 vertical rules. Cells
/// contain short text labels.
fn grid_pdf() -> Vec<u8> {
    let content = "\
q 1 w \
100 600 m 500 600 l S \
100 560 m 500 560 l S \
100 520 m 500 520 l S \
100 480 m 500 480 l S \
100 480 m 100 600 l S \
230 480 m 230 600 l S \
370 480 m 370 600 l S \
500 480 m 500 600 l S \
Q \
BT /F1 11 Tf 110 570 Td (A1) Tj ET \
BT /F1 11 Tf 240 570 Td (B1) Tj ET \
BT /F1 11 Tf 380 570 Td (C1) Tj ET \
BT /F1 11 Tf 110 530 Td (A2) Tj ET \
BT /F1 11 Tf 240 530 Td (B2) Tj ET \
BT /F1 11 Tf 380 530 Td (C2) Tj ET \
BT /F1 11 Tf 110 490 Td (A3) Tj ET \
BT /F1 11 Tf 240 490 Td (B3) Tj ET \
BT /F1 11 Tf 380 490 Td (C3) Tj ET \
BT /F1 11 Tf 72 700 Td (Grid anchor) Tj ET";
    build_pdf(content)
}

/// A 20×20 mesh of 0.85pt-tall vertical micro-strokes packed into an
/// ~17pt × 17pt region, plus anchor text. This mimics the signature /
/// barcode texture found in Quebec government-issued PDFs at
/// `/after/4053213550.md`: individual segments are below the ASCII-
/// calibrated half-cell threshold and MUST be dropped, otherwise they
/// stack into a spurious `|||||||` cluster.
fn micro_mesh_pdf() -> Vec<u8> {
    let mut content = String::from("q 0.3 w ");
    for xi in 0..20 {
        let x = 100.0 + xi as f32 * 0.85;
        for yi in 0..20 {
            let y = 700.0 + yi as f32 * 0.85;
            content.push_str(&format!("{x:.3} {y:.3} m {x:.3} {:.3} l S ", y + 0.85));
        }
    }
    content.push_str("Q ");
    content.push_str("BT /F1 12 Tf 72 500 Td (Mesh anchor text) Tj ET ");
    content.push_str("BT /F1 12 Tf 72 480 Td (Second anchor row) Tj ET");
    build_pdf(&content)
}

/// A page with ONLY a strongly oblique (45°) stroked line plus anchor text.
/// The oblique line must be dropped (no spurious `-` or `|` far from text).
fn oblique_line_pdf() -> Vec<u8> {
    let content = "\
q 1 w \
100 500 m 500 700 l S \
Q \
BT /F1 12 Tf 72 400 Td (Oblique anchor) Tj ET \
BT /F1 12 Tf 72 380 Td (Second anchor) Tj ET";
    build_pdf(content)
}

/// A page mimicking the bottom-of-page geometry from the wild
/// `weird_invoice.pdf`: four footer rows at uniform 7pt spacing with the
/// bottommost row anchored at `y = 100` (so its normalized y is exactly
/// `1.0`), plus a header anchor far above.
///
/// Each row has a unique left-column label (`RowNA`) and right-column
/// label (`RowNB`) so we can tell them apart in the rendered output.
///
/// Under the pre-fix `y * target_rows` quantization with clamp-to-`N-1`,
/// the bottom two rows (Row3 at `ny ≈ 0.989`, Row4 at `ny = 1.0`) both
/// landed on grid row `N - 1` — Row3 by direct rounding, Row4 via the
/// clamp from `N`. Spatial placement is last-write-wins, so Row3A/B got
/// *overwritten* by Row4A/B and disappeared from the output. The fix
/// (`y * (target_rows - 1)`) lands `y = 1.0` exactly on `N - 1` with no
/// clamp, keeping every distinct input row on a distinct grid row.
///
/// Rows 1 and 2 sit comfortably above the pre-fix collision zone
/// `[1 − 1.5/N, 1.0]`, so their survival across both pre-fix and post-fix
/// renderers isolates the test to Bug A (boundary clamp) rather than the
/// orthogonal "rows-too-close-for-the-grid" issue.
fn footer_rows_pdf() -> Vec<u8> {
    let content = "\
BT /F1 12 Tf 72 720 Td (Page top anchor) Tj ET \
BT /F1 12 Tf 72 700 Td (Body line one) Tj ET \
BT /F1 12 Tf 72 680 Td (Body line two) Tj ET \
BT /F1 8 Tf 72 121 Td (Row1A) Tj ET \
BT /F1 8 Tf 400 121 Td (Row1B) Tj ET \
BT /F1 8 Tf 72 114 Td (Row2A) Tj ET \
BT /F1 8 Tf 400 114 Td (Row2B) Tj ET \
BT /F1 8 Tf 72 107 Td (Row3A) Tj ET \
BT /F1 8 Tf 400 107 Td (Row3B) Tj ET \
BT /F1 8 Tf 72 100 Td (Row4A) Tj ET \
BT /F1 8 Tf 400 100 Td (Row4B) Tj ET";
    build_pdf(content)
}

// ---------------------------------------------------------------------------
// Small predicates
// ---------------------------------------------------------------------------

fn output_lines(pages: &[spatial::SpatialPage]) -> Vec<String> {
    pages.iter().flat_map(|p| p.lines.clone()).collect()
}

fn md_output_lines(pages: &[markdown::MarkdownPage]) -> Vec<String> {
    pages.iter().flat_map(|p| p.lines.clone()).collect()
}

fn longest_run(s: &str, c: char) -> usize {
    let mut best = 0;
    let mut cur = 0;
    for ch in s.chars() {
        if ch == c {
            cur += 1;
            if cur > best {
                best = cur;
            }
        } else {
            cur = 0;
        }
    }
    best
}

// ---------------------------------------------------------------------------
// Spatial renderer
// ---------------------------------------------------------------------------

#[test]
fn spatial_horizontal_line_renders_as_dashes() {
    let pdf = horizontal_line_pdf();
    let pages = spatial::render_from_bytes(&pdf, &SpatialConfig::default());
    assert!(!pages.is_empty(), "expected at least one page");

    let lines = output_lines(&pages);
    let max_dash_run = lines.iter().map(|l| longest_run(l, '-')).max().unwrap_or(0);
    assert!(
        max_dash_run >= 5,
        "expected a run of '-' characters for the horizontal rule, got {max_dash_run} dashes max. Full output: {lines:?}",
    );
}

#[test]
fn spatial_vertical_line_renders_as_pipes() {
    let pdf = vertical_line_pdf();
    let pages = spatial::render_from_bytes(&pdf, &SpatialConfig::default());
    assert!(!pages.is_empty(), "expected at least one page");

    let lines = output_lines(&pages);
    let pipe_rows = lines.iter().filter(|l| l.contains('|')).count();
    assert!(
        pipe_rows >= 2,
        "expected '|' to appear on multiple rows for the vertical rule, got {pipe_rows}. Full output: {lines:?}",
    );
}

#[test]
fn spatial_stroked_rect_renders_all_four_edges() {
    let pdf = stroked_rect_pdf();
    let pages = spatial::render_from_bytes(&pdf, &SpatialConfig::default());
    assert!(!pages.is_empty());

    let lines = output_lines(&pages);
    // Horizontal edges → at least two different rows with dash runs.
    let dash_rows: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| {
            if longest_run(l, '-') >= 3 {
                Some(i)
            } else {
                None
            }
        })
        .collect();
    assert!(
        dash_rows.len() >= 2,
        "expected top and bottom edges of the rect to produce dash runs on at least 2 rows, got rows {dash_rows:?}. Full output: {lines:?}",
    );
    // Vertical edges → at least one row has two `|`s (left and right).
    let two_pipes = lines.iter().any(|l| l.matches('|').count() >= 2);
    assert!(
        two_pipes,
        "expected at least one row containing both vertical edges of the rect as '|'. Full output: {lines:?}",
    );
}

#[test]
fn spatial_grid_preserves_both_axes() {
    let pdf = grid_pdf();
    let pages = spatial::render_from_bytes(&pdf, &SpatialConfig::default());
    assert!(!pages.is_empty());

    let lines = output_lines(&pages);
    let dash_rows = lines.iter().filter(|l| longest_run(l, '-') >= 5).count();
    let pipe_rows = lines.iter().filter(|l| l.contains('|')).count();

    // 4 horizontal rulings → at least 3 distinct rows with dashes (some
    // may merge if target_rows is low).
    assert!(
        dash_rows >= 3,
        "expected multiple rows of dashes for the grid, got {dash_rows}. Full output: {lines:?}",
    );
    // 4 vertical rulings → at least several rows carry pipe characters.
    assert!(
        pipe_rows >= 3,
        "expected multiple rows of pipes for the grid, got {pipe_rows}. Full output: {lines:?}",
    );
    // Cell labels must survive rule drawing (text wins at collisions).
    let joined = lines.join("\n");
    assert!(
        joined.contains("A1") && joined.contains("B2") && joined.contains("C3"),
        "cell labels should remain visible after rules rendering. Full output: {lines:?}",
    );
}

#[test]
fn spatial_micro_mesh_does_not_produce_barcode_artifact() {
    // Regression: before ASCII-calibrated row sizing, this mesh of tiny
    // vertical strokes (each 0.85pt tall) rendered as ~10 stacked rows
    // of `|||||||` at the top of government-issued PDFs. On the fixed
    // grid, every individual segment falls below the 0.5-row threshold
    // and the mesh disappears entirely (geometrically faithful: the
    // whole region is smaller than a single terminal character).
    let pdf = micro_mesh_pdf();
    let pages = spatial::render_from_bytes(&pdf, &SpatialConfig::default());
    assert!(!pages.is_empty());

    let lines = output_lines(&pages);
    let max_pipe_run = lines.iter().map(|l| longest_run(l, '|')).max().unwrap_or(0);
    assert!(
        max_pipe_run < 3,
        "micro mesh must not produce a '|' cluster on the calibrated grid, got max run of {max_pipe_run}. Full output: {lines:?}",
    );
    let pipe_rows = lines.iter().filter(|l| l.contains('|')).count();
    assert!(
        pipe_rows < 3,
        "micro mesh must not populate many rows with '|', got {pipe_rows}. Full output: {lines:?}",
    );
    // No spurious dashes either — each micro-segment is narrower than
    // one column, so no horizontal rule should survive.
    let max_dash_run = lines.iter().map(|l| longest_run(l, '-')).max().unwrap_or(0);
    assert!(
        max_dash_run < 3,
        "micro mesh must not produce a '-' run either, got max run of {max_dash_run}. Full output: {lines:?}",
    );
    // The anchor text must still be visible — we dropped the mesh, not
    // the page.
    let joined = lines.join("\n");
    assert!(
        joined.contains("Mesh anchor text"),
        "anchor text should survive; full output: {lines:?}",
    );
}

#[test]
fn spatial_oblique_line_is_dropped() {
    let pdf = oblique_line_pdf();
    let pages = spatial::render_from_bytes(&pdf, &SpatialConfig::default());
    assert!(!pages.is_empty());

    let lines = output_lines(&pages);
    // The oblique line must NOT introduce rows of dashes or pipes.
    // Anchor text ("Oblique anchor") contains no `-` or `|` itself.
    for l in &lines {
        assert!(
            longest_run(l, '-') < 3,
            "unexpected dash run from oblique line: {l:?}",
        );
        assert!(!l.contains('|'), "unexpected pipe from oblique line: {l:?}",);
    }
}

// ---------------------------------------------------------------------------
// Markdown renderer (same fixtures, same assertions)
// ---------------------------------------------------------------------------

#[test]
fn markdown_horizontal_line_renders_as_dashes() {
    let pdf = horizontal_line_pdf();
    let pages = markdown::render_from_bytes(&pdf, &MarkdownConfig::default());
    assert!(!pages.is_empty());

    let lines = md_output_lines(&pages);
    let max_dash_run = lines.iter().map(|l| longest_run(l, '-')).max().unwrap_or(0);
    assert!(
        max_dash_run >= 5,
        "expected a '-' run in markdown output, got {max_dash_run}. Full output: {lines:?}",
    );
}

#[test]
fn markdown_vertical_line_renders_as_pipes() {
    let pdf = vertical_line_pdf();
    let pages = markdown::render_from_bytes(&pdf, &MarkdownConfig::default());
    assert!(!pages.is_empty());

    let lines = md_output_lines(&pages);
    let pipe_rows = lines.iter().filter(|l| l.contains('|')).count();
    assert!(
        pipe_rows >= 2,
        "expected multiple '|' rows in markdown output, got {pipe_rows}. Full output: {lines:?}",
    );
}

#[test]
fn markdown_rules_do_not_gain_formatting() {
    // Rules never carry markdown emphasis — a horizontal ruler stays plain
    // `-` characters, even when the surrounding text is bold / italic /
    // linked elsewhere.
    let pdf = horizontal_line_pdf();
    let pages = markdown::render_from_bytes(&pdf, &MarkdownConfig::default());
    let lines = md_output_lines(&pages);
    let dash_line = lines
        .iter()
        .find(|l| longest_run(l, '-') >= 5)
        .expect("expected a dash line");
    // No `**` / `*` / `[` emphasis around the dash run.
    // (The underlying char is `-`; formatting would wrap it with markers.)
    let dashes_only = dash_line.chars().filter(|c| *c == '-').collect::<String>();
    assert!(
        dashes_only.len() >= 5,
        "expected pure dash run (no formatting wrapping), got line {dash_line:?}",
    );
}

#[test]
fn markdown_micro_mesh_does_not_produce_barcode_artifact() {
    // Mirror of the spatial regression guard: the markdown renderer
    // shares the same calibration, so the same mesh must stay dropped.
    let pdf = micro_mesh_pdf();
    let pages = markdown::render_from_bytes(&pdf, &MarkdownConfig::default());
    assert!(!pages.is_empty());

    let lines = md_output_lines(&pages);
    let max_pipe_run = lines.iter().map(|l| longest_run(l, '|')).max().unwrap_or(0);
    assert!(
        max_pipe_run < 3,
        "micro mesh must not produce a '|' cluster in markdown output, got max run of {max_pipe_run}. Full output: {lines:?}",
    );
    let pipe_rows = lines.iter().filter(|l| l.contains('|')).count();
    assert!(
        pipe_rows < 3,
        "micro mesh must not populate many rows with '|' in markdown output, got {pipe_rows}. Full output: {lines:?}",
    );
    let max_dash_run = lines.iter().map(|l| longest_run(l, '-')).max().unwrap_or(0);
    assert!(
        max_dash_run < 3,
        "micro mesh must not produce a '-' run in markdown output, got max run of {max_dash_run}. Full output: {lines:?}",
    );
    let joined = lines.join("\n");
    assert!(
        joined.contains("Mesh anchor text"),
        "anchor text should survive; full output: {lines:?}",
    );
}

#[test]
fn markdown_oblique_line_is_dropped() {
    let pdf = oblique_line_pdf();
    let pages = markdown::render_from_bytes(&pdf, &MarkdownConfig::default());
    let lines = md_output_lines(&pages);
    for l in &lines {
        assert!(
            longest_run(l, '-') < 3,
            "unexpected dash run from oblique line (markdown): {l:?}",
        );
        assert!(
            !l.contains('|'),
            "unexpected pipe from oblique line (markdown): {l:?}",
        );
    }
}

// ---------------------------------------------------------------------------
// Bottom-boundary row collision (weird_invoice.pdf footer regression)
// ---------------------------------------------------------------------------
//
// These two tests pin the Y-quantization convention that sits at the heart of
// the grid renderers. When the four footer rows of a PDF are packed tightly
// at the bottom of the page, the old `y * target_rows` mapping (clamped from
// `target_rows` down to `target_rows - 1` for `y = 1.0`) sent the two
// bottommost rows to the same integer grid row. Spatial placement is
// last-write-wins, so the penultimate row's text was overwritten by the
// bottommost row and disappeared from the output entirely. The fix —
// `y * (target_rows - 1)` — lands `y = 1.0` exactly on `target_rows - 1`
// with no clamp, keeping distinct PDF rows on distinct grid rows.

fn assert_footer_rows_all_present_and_distinct(lines: &[String]) {
    let joined = lines.join("\n");
    for label in [
        "Row1A", "Row2A", "Row3A", "Row4A", "Row1B", "Row2B", "Row3B", "Row4B",
    ] {
        assert!(
            joined.contains(label),
            "{label} disappeared from output, indicating row collapse near the bottom boundary. Full output: {lines:?}",
        );
    }

    // Each RowNA label must appear on a distinct output line — if two
    // rows collapsed to the same grid row, last-write-wins would leave at
    // most one visible *and* the surviving line would contain exactly one
    // label, not two. So the sharper check is per-line multiplicity plus
    // total line count covering all four labels.
    let labels = ["Row1A", "Row2A", "Row3A", "Row4A"];
    let mut lines_with_label: Vec<usize> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let hits = labels.iter().filter(|t| line.contains(*t)).count();
        assert!(
            hits <= 1,
            "line {i} carries multiple footer labels — rows fused on one grid row. Line: {line:?}. Full output: {lines:?}",
        );
        if hits == 1 {
            lines_with_label.push(i);
        }
    }
    assert_eq!(
        lines_with_label.len(),
        4,
        "expected exactly 4 distinct output lines carrying the 4 footer Left-column labels, got {}. Full output: {lines:?}",
        lines_with_label.len(),
    );
}

/// Target-cols value calibrated to yield `target_rows ≈ 130`, a grid coarse
/// enough that a 7pt row spacing is comfortably above 1 cell (safe from
/// the orthogonal "rows-too-close-for-the-grid" issue) AND fine enough
/// that Row3's `ny ≈ 0.989` still falls inside the pre-fix collision zone
/// `[1 − 1.5/N, 1.0]`. The exact value of `target_rows` is derived by the
/// renderer from this override × content aspect ratio, so this value is
/// the knob that keeps the test in the Bug-A-exposing regime.
const FOOTER_TEST_TARGET_COLS: usize = 140;

#[test]
fn spatial_bottom_boundary_rows_do_not_collide() {
    let pdf = footer_rows_pdf();
    let config = SpatialConfig {
        target_cols_override: FOOTER_TEST_TARGET_COLS,
        profile: false,
    };
    let pages = spatial::render_from_bytes(&pdf, &config);
    assert!(!pages.is_empty(), "expected at least one page");

    let lines = output_lines(&pages);
    assert_footer_rows_all_present_and_distinct(&lines);
}

#[test]
fn markdown_bottom_boundary_rows_do_not_collide() {
    let pdf = footer_rows_pdf();
    let config = MarkdownConfig {
        target_cols_override: FOOTER_TEST_TARGET_COLS,
        profile: false,
    };
    let pages = markdown::render_from_bytes(&pdf, &config);
    assert!(!pages.is_empty(), "expected at least one page");

    let lines = md_output_lines(&pages);
    assert_footer_rows_all_present_and_distinct(&lines);
}
