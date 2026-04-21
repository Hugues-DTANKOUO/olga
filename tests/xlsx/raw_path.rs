use olga::formats::xlsx::XlsxDecoder;
use olga::model::*;
use olga::traits::FormatDecoder;

use crate::support::xlsx::{
    build_manifest_fraction_canary_xlsx, build_watchlist_canary_xlsx,
    build_xlsx_with_comments_on_empty_cells, build_xlsx_with_formula_without_cached_value,
    build_xlsx_with_hidden_first_row_heuristic_header, build_xlsx_with_hidden_rows_and_cols,
    build_xlsx_with_missing_sheet_part, build_xlsx_with_native_table,
    build_xlsx_with_sheet_hyperlink,
};

#[test]
fn decode_missing_sheet_warns_and_continues() {
    let data = build_xlsx_with_missing_sheet_part();
    let decoder = XlsxDecoder::new();
    let result = decoder.decode(data.clone()).unwrap();

    assert_eq!(result.metadata.page_count, 2);
    // Sheet title heading ("Present") + one data cell ("A1"). The missing
    // "Missing" sheet contributes zero primitives and surfaces a warning.
    assert_eq!(result.primitives.len(), 2);
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.kind == olga::error::WarningKind::MissingPart)
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.kind == olga::error::WarningKind::PartialExtraction)
    );
}

#[test]
fn decode_native_table_headers_override_sheet_heuristic() {
    let data = build_xlsx_with_native_table();

    let decoder = XlsxDecoder::new();
    let result = decoder.decode(data.clone()).unwrap();

    let header_cells: Vec<_> = result
        .primitives
        .iter()
        .filter(|primitive| {
            primitive
                .hints
                .iter()
                .any(|hint| matches!(hint.kind, HintKind::TableHeader { .. }))
        })
        .collect();
    assert_eq!(header_cells.len(), 2);

    let table_cells: Vec<_> = result
        .primitives
        .iter()
        .filter(|primitive| {
            primitive
                .hints
                .iter()
                .any(|hint| matches!(hint.kind, HintKind::TableCell { .. }))
        })
        .collect();
    assert_eq!(table_cells.len(), 2);

    let notes = result
        .primitives
        .iter()
        .find(|primitive| match &primitive.kind {
            PrimitiveKind::Text { content, .. } => content == "Notes",
            _ => false,
        })
        .unwrap();
    assert!(notes.hints.is_empty());

    let name = result
        .primitives
        .iter()
        .find(|primitive| match &primitive.kind {
            PrimitiveKind::Text { content, .. } => content == "Name",
            _ => false,
        })
        .unwrap();
    assert!(
        name.hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::TableHeader { col: 0 }))
    );

    let age = result
        .primitives
        .iter()
        .find(|primitive| match &primitive.kind {
            PrimitiveKind::Text { content, .. } => content == "Age",
            _ => false,
        })
        .unwrap();
    assert!(
        age.hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::TableHeader { col: 1 }))
    );

    let alice = result
        .primitives
        .iter()
        .find(|primitive| match &primitive.kind {
            PrimitiveKind::Text { content, .. } => content == "Alice",
            _ => false,
        })
        .unwrap();
    assert!(alice.hints.iter().any(|hint| {
        matches!(
            hint.kind,
            HintKind::TableCell {
                row: 1,
                col: 0,
                rowspan: 1,
                colspan: 1,
            }
        )
    }));
    assert!(
        !result
            .warnings
            .iter()
            .any(|warning| { warning.kind == olga::error::WarningKind::HeuristicInference })
    );
}

#[test]
fn decode_hidden_rows_and_cols_are_surfaced_like_pandas() {
    // `<col hidden="1"/>` and `<row hidden="1"/>` are UI-visibility signals
    // in OOXML — they hide a row/column in Excel's grid view but do not
    // remove the values from the workbook. Pandas / openpyxl / calamine /
    // Apache POI all expose hidden cells unconditionally because the data
    // belongs to the sheet regardless of rendering. Dropping them silently
    // omits authored content (e.g. a hidden tax column in a budget) and
    // diverges from every mainstream extractor. We therefore surface them
    // too, keeping the hidden flag as informational metadata only.
    let data = build_xlsx_with_hidden_rows_and_cols();

    let decoder = XlsxDecoder::new();
    let result = decoder.decode(data.clone()).unwrap();

    let contents: Vec<_> = result
        .primitives
        .iter()
        .filter_map(|primitive| match &primitive.kind {
            PrimitiveKind::Text { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect();

    // Sheet title heading ("Hidden") + 3 rows × 3 cols = 10 primitives.
    assert_eq!(result.primitives.len(), 10);
    assert_eq!(
        contents,
        vec![
            "Hidden", "ID", "Name", "City", "1", "Alice", "Paris", "2", "Bob", "London",
        ]
    );

    assert!(!result.warnings.iter().any(|warning| {
        warning.kind == olga::error::WarningKind::PartialExtraction
            && warning.message.contains("hidden rows/columns")
    }));

    // Hidden column B ("Name") keeps its natural mid-grid X position.
    let name = result
        .primitives
        .iter()
        .find(|primitive| match &primitive.kind {
            PrimitiveKind::Text { content, .. } => content == "Name",
            _ => false,
        })
        .unwrap();
    assert!((name.bbox.x - (1.0 / 3.0)).abs() < 0.0001);

    // Hidden row 2 ("Alice") keeps its natural mid-grid Y position.
    let alice = result
        .primitives
        .iter()
        .find(|primitive| match &primitive.kind {
            PrimitiveKind::Text { content, .. } => content == "Alice",
            _ => false,
        })
        .unwrap();
    assert!((alice.bbox.y - (1.0 / 3.0)).abs() < 0.0001);
}

#[test]
fn decode_hidden_first_row_still_anchors_header_heuristic() {
    // Hidden rows are UI-only metadata (see SheetContext::new); the header
    // fallback heuristic therefore considers the literal first row of the
    // sheet, even when it carries `hidden="1"`. Pandas and openpyxl behave
    // the same way — hidden rows participate in column inference because
    // the values remain part of the workbook.
    let data = build_xlsx_with_hidden_first_row_heuristic_header();

    let decoder = XlsxDecoder::new();
    let result = decoder.decode(data.clone()).unwrap();

    assert!(result.warnings.iter().any(|warning| {
        warning.kind == olga::error::WarningKind::HeuristicInference
            && warning.message.contains("Sheet 'HiddenHeaderFallback'")
            && warning.message.contains("row 1")
    }));

    let shadow = result
        .primitives
        .iter()
        .find(|primitive| match &primitive.kind {
            PrimitiveKind::Text { content, .. } => content == "shadow",
            _ => false,
        })
        .unwrap();
    assert!(
        shadow
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::TableHeader { col: 0 }))
    );

    let ghost = result
        .primitives
        .iter()
        .find(|primitive| match &primitive.kind {
            PrimitiveKind::Text { content, .. } => content == "ghost",
            _ => false,
        })
        .unwrap();
    assert!(
        ghost
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::TableHeader { col: 1 }))
    );

    let alice = result
        .primitives
        .iter()
        .find(|primitive| match &primitive.kind {
            PrimitiveKind::Text { content, .. } => content == "Alice",
            _ => false,
        })
        .unwrap();
    assert!(alice.hints.iter().any(|hint| {
        matches!(
            hint.kind,
            HintKind::TableCell {
                row: 2,
                col: 0,
                rowspan: 1,
                colspan: 1,
            }
        )
    }));
}

#[test]
fn decode_sheet_hyperlink_emits_link_hint_on_raw_path() {
    let data = build_xlsx_with_sheet_hyperlink();

    let decoder = XlsxDecoder::new();
    let result = decoder.decode(data.clone()).unwrap();

    // Sheet title heading + the single "Portal" data cell.
    assert_eq!(result.primitives.len(), 2);
    let portal = result
        .primitives
        .iter()
        .find(|primitive| match &primitive.kind {
            PrimitiveKind::Text { content, .. } => content == "Portal",
            _ => false,
        })
        .expect("Portal cell surfaces as a text primitive");
    assert!(portal.hints.iter().any(|hint| matches!(
        &hint.kind,
        HintKind::Link { url } if url == "https://example.com/portal"
    )));
}

#[test]
fn decode_surfaces_comments_attached_to_empty_cells() {
    // A reviewer left notes on C3 (no `<c>` element in the sheet XML) and
    // on B2 (a self-closing style-only `<c/>` element). Before the union-
    // iteration fix the streaming pass emitted only cells that appeared
    // in the XML *with a value*, so both annotations were silently
    // dropped. With the fix the grid grows to cover every commented cell
    // and the emit pass visits both the `<c>`-backed cells and the
    // comment-only cells in a single pass.
    let data = build_xlsx_with_comments_on_empty_cells();

    let decoder = XlsxDecoder::new();
    let result = decoder.decode(data.clone()).unwrap();

    let contents: Vec<&str> = result
        .primitives
        .iter()
        .filter_map(|primitive| match &primitive.kind {
            PrimitiveKind::Text { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect();

    // Sheet title ("Notes") + A1 ("Anchor") + two annotation-only cells.
    assert_eq!(result.primitives.len(), 4);
    assert!(
        contents.contains(&"Anchor"),
        "anchor data cell must still surface (got {:?})",
        contents
    );
    // Comment rendering prefixes the author (here "Reviewer:" because the
    // fixture's `<author>Reviewer</author>` resolves inline). The exact
    // author rendering is an orthogonal contract owned by
    // `metadata::comments`; this test just pins that the *annotation-only
    // cell* contract — self-closing `<c/>` and pure orphan — both surface.
    assert!(
        contents.iter().any(|c| c.contains("style-only cell note")),
        "B2 self-closing `<c/>` with comment must surface (got {:?})",
        contents
    );
    assert!(
        contents.iter().any(|c| c.contains("orphan annotation")),
        "C3 cell present only as a comment must surface (got {:?})",
        contents
    );
}

#[test]
fn decode_formula_without_cached_value_warns_on_raw_path() {
    let data = build_xlsx_with_formula_without_cached_value();

    let decoder = XlsxDecoder::new();
    let result = decoder.decode(data.clone()).unwrap();

    // Sheet title heading + the single "Total" cell. The formula cell
    // without a cached value contributes only a warning.
    assert_eq!(result.primitives.len(), 2);
    let total = result
        .primitives
        .iter()
        .find(|primitive| match &primitive.kind {
            PrimitiveKind::Text { content, .. } => content == "Total",
            _ => false,
        })
        .expect("Total cell surfaces as a text primitive");
    assert!(matches!(&total.kind, PrimitiveKind::Text { .. }));
    assert!(result.warnings.iter().any(|warning| {
        warning.kind == olga::error::WarningKind::PartialExtraction
            && warning.message.contains(
                "Formula cell in sheet 'Formulas' had no cached value on the raw XLSX path",
            )
    }));
}

/// Watchlist canary: end-to-end validation that the POI-aligned
/// renderer produces every defect-free rendering surfaced on the
/// production `watchlist.xlsx` document faithfully, in a single decode
/// pass through the public XLSX extractor.
///
/// Each canary corresponds to a specific failure on the legacy pipeline
/// that the POI-aligned renderer replaces. Pinning all five in one test
/// guards against piecemeal regressions where one defect re-emerges
/// while the others stay green:
///
/// | Cell | Format code            | Raw    | Expected   |
/// |------|------------------------|--------|------------|
/// | B1   | `0.00E+00`             | 2.918e13 | `2.92E+13` |
/// | B2   | `h:mm AM/PM`           | 0.395833 | `9:30 AM`  |
/// | B3   | `+0"bps";-0"bps"`      | 2      | `+2bps`    |
/// | B4   | `+0"bps";-0"bps"`      | -1     | `-1bps`    |
/// | B5   | `0.0"x"`               | 62.4   | `62.4x`    |
///
/// The sixth canary — the comment on `Watchlist!F9` — pins the union-
/// iteration fix from #46 against a workbook that *also* exercises the
/// number-format pipeline, so a regression in either subsystem trips
/// this single assertion.
#[test]
fn decode_watchlist_canaries_render_via_poi_aligned_pipeline() {
    let data = build_watchlist_canary_xlsx();

    let decoder = XlsxDecoder::new();
    let result = decoder.decode(data.clone()).unwrap();

    let contents: Vec<&str> = result
        .primitives
        .iter()
        .filter_map(|primitive| match &primitive.kind {
            PrimitiveKind::Text { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect();

    // (a) Scientific notation: 2.918e13 against `0.00E+00` must NOT
    //     collapse to `29180000000000.00` (the legacy classifier's
    //     `Number { decimals: 2 }` rendering).
    assert!(
        contents.contains(&"2.92E+13"),
        "scientific canary must render `2.92E+13` (got {:?})",
        contents
    );
    assert!(
        !contents.iter().any(|c| c.contains("29180000000000")),
        "scientific canary must not collapse to the integer form (got {:?})",
        contents
    );

    // (b) Time-of-day: 9.5 hours / 24 against `h:mm AM/PM` must NOT
    //     emit the ISO datetime form the legacy date renderer used.
    assert!(
        contents.contains(&"9:30 AM"),
        "h:mm AM/PM canary must render `9:30 AM` (got {:?})",
        contents
    );
    assert!(
        !contents.iter().any(|c| c.contains("1899-12-31")),
        "h:mm AM/PM canary must not leak the 1899-12-31 epoch anchor (got {:?})",
        contents
    );

    // (c) Custom sign-bearing positive section: `+0"bps";-0"bps"`
    //     against `+2`. The body's `+` literal must render the sign.
    assert!(
        contents.contains(&"+2bps"),
        "positive bps canary must render `+2bps` (got {:?})",
        contents
    );

    // (d) Custom sign-bearing negative section: against `-1`. The
    //     legacy pipeline emitted `-+1bps` because it both stripped the
    //     section dispatch and added its own minus. POI's hasSign() rule
    //     fixes this — single sign only.
    assert!(
        contents.contains(&"-1bps"),
        "negative bps canary must render `-1bps` (got {:?})",
        contents
    );
    assert!(
        !contents
            .iter()
            .any(|c| c.contains("-+") || c.contains("+-")),
        "no rendered cell may carry a double-sign artifact (got {:?})",
        contents
    );

    // (e) Literal suffix: `0.0"x"` against 62.4 — the `x` literal must
    //     survive section parsing.
    assert!(
        contents.contains(&"62.4x"),
        "suffix-x canary must render `62.4x` (got {:?})",
        contents
    );

    // (f) F9 comment: the bare cell has no `<c>` element, so without
    //     the union-iteration emit pass the annotation would be silently
    //     dropped. The exact rendering format is owned by the comments
    //     subsystem; this assertion just pins that the body text reaches
    //     the primitive stream.
    assert!(
        contents
            .iter()
            .any(|c| c.contains("watchlist F9 review note")),
        "F9 comment must surface as a primitive (got {:?})",
        contents
    );

    // The format pipeline must not raise PartialExtraction or
    // MalformedContent on any canary — clean renderings only. Comment
    // / structural warnings from other subsystems are out of scope.
    let format_warnings: Vec<&olga::error::Warning> = result
        .warnings
        .iter()
        .filter(|w| w.message.contains("xlsx number format"))
        .collect();
    assert!(
        format_warnings.is_empty(),
        "no number-format warnings expected on the canary set (got {:?})",
        format_warnings
    );
}

/// #48 regression: the imperial-fraction format `# ?/8"` from the
/// production `manifest.xlsx` document must render as mixed fractions
/// (e.g. `96 1/8"`) rather than falling back to the raw decimal.
///
/// The legacy pipeline ignored the `/` fraction marker entirely and
/// emitted `96.125`; this assertion pins the POI-aligned fraction
/// renderer end-to-end, including the inch-mark suffix literal and the
/// "no fractional part when it rounds to 0/8" case on row 2.
#[test]
fn decode_manifest_fraction_canaries_render_via_poi_aligned_pipeline() {
    let data = build_manifest_fraction_canary_xlsx();

    let decoder = XlsxDecoder::new();
    let result = decoder.decode(data.clone()).unwrap();

    let contents: Vec<&str> = result
        .primitives
        .iter()
        .filter_map(|primitive| match &primitive.kind {
            PrimitiveKind::Text { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect();

    // The five expected renderings. Row 1 and row 4 both render
    // `96 1/8"` (duplicates are fine — the assertion just checks
    // presence in the primitive stream). Row 3 reduces `2/8` → `1/4`
    // per the OpenXML convention shared by Excel and LibreOffice.
    for expected in ["96 1/8\"", "96\"", "96 1/4\"", "96 3/8\""] {
        assert!(
            contents.contains(&expected),
            "fraction canary must render {:?} (got {:?})",
            expected,
            contents
        );
    }

    // `2/8` must not leak through: a regression that disables GCD
    // reduction would render it verbatim, which Excel and LibreOffice
    // never do for `?/8` formats.
    assert!(
        !contents.iter().any(|c| c.contains("2/8")),
        "fraction canary must reduce `2/8` → `1/4` (got {:?})",
        contents
    );

    // The raw decimal form must not leak through — if any of these
    // appears, the fraction grammar fell back to General.
    for forbidden in ["96.125", "96.25", "96.375", "96.000"] {
        assert!(
            !contents.iter().any(|c| c.contains(forbidden)),
            "fraction canary must not leak raw decimal {:?} (got {:?})",
            forbidden,
            contents
        );
    }

    // No number-format warnings: the grammar is now supported natively,
    // so the engine must not surface `FormatFeatureFraction` for any of
    // these rows.
    let format_warnings: Vec<&olga::error::Warning> = result
        .warnings
        .iter()
        .filter(|w| w.message.contains("xlsx number format"))
        .collect();
    assert!(
        format_warnings.is_empty(),
        "no number-format warnings expected on the manifest canary set (got {:?})",
        format_warnings
    );
}
