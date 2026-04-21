use super::*;
use crate::formats::pdf::types::RawChar;
use crate::model::geometry::{PageDimensions, RawBox};
use crate::model::{Color, TextDirection, ValueOrigin};

fn letter_page() -> PageDimensions {
    PageDimensions::new(RawBox::new(0.0, 0.0, 612.0, 792.0), None, 0)
}

fn make_char(text: &str, x: f32, y: f32, width: f32, font: &str, size: f32) -> RawChar {
    RawChar {
        text: text.chars().next().unwrap_or(' '),
        raw_x: x,
        raw_y: y,
        raw_width: width,
        raw_height: size,
        font_name: font.to_string(),
        font_size: size,
        is_bold: false,
        is_italic: false,
        color_r: 0.0,
        color_g: 0.0,
        color_b: 0.0,
        is_monospace: false,
        origin_x: x,
        origin_y: y,
        advance_width: width,
        rotation_degrees: 0.0,
        mcid: None,
    }
}

fn make_rotated_char(
    text: &str,
    x: f32,
    y: f32,
    width: f32,
    font: &str,
    size: f32,
    rotation_degrees: f32,
) -> RawChar {
    RawChar {
        rotation_degrees,
        ..make_char(text, x, y, width, font, size)
    }
}

#[test]
fn clean_font_name_strips_subset_prefix() {
    assert_eq!(clean_font_name("ABCDEF+Arial"), "Arial");
    assert_eq!(clean_font_name("Arial"), "Arial");
    assert_eq!(
        clean_font_name("ABCDEF+TimesNewRoman-Bold"),
        "TimesNewRoman-Bold"
    );
}

#[test]
fn bold_italic_detection() {
    assert!(is_bold_font("Arial-Bold"));
    assert!(is_bold_font("TimesNewRoman,Bold"));
    assert!(is_bold_font("Helvetica-BoldOblique"));
    assert!(!is_bold_font("Arial"));

    assert!(is_italic_font("Arial-Italic"));
    assert!(is_italic_font("TimesNewRoman-Oblique"));
    assert!(!is_italic_font("Arial"));
}

#[test]
fn group_empty_chars() {
    let dims = letter_page();
    let spans = group_chars_into_spans(&[], &dims);
    assert!(spans.is_empty());
}

#[test]
fn group_single_word() {
    let dims = letter_page();
    let chars = vec![
        make_char("H", 72.0, 720.0, 7.0, "Helvetica", 12.0),
        make_char("i", 79.0, 720.0, 4.0, "Helvetica", 12.0),
    ];

    let spans = group_chars_into_spans(&chars, &dims);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].content, "Hi");
    assert_eq!(spans[0].font_name, "Helvetica");
    assert_eq!(spans[0].font_size, 12.0);
}

#[test]
fn group_breaks_on_font_change() {
    let dims = letter_page();
    let chars = vec![
        make_char("A", 72.0, 720.0, 7.0, "Helvetica", 12.0),
        RawChar {
            is_bold: true,
            font_name: "Arial-Bold".to_string(),
            ..make_char("B", 79.0, 720.0, 7.0, "Arial-Bold", 12.0)
        },
    ];

    let spans = group_chars_into_spans(&chars, &dims);
    assert_eq!(spans.len(), 2);
    assert_eq!(spans[0].content, "A");
    assert_eq!(spans[1].content, "B");
}

#[test]
fn group_breaks_on_mcid_change() {
    let dims = letter_page();
    let chars = vec![
        RawChar {
            mcid: Some(1),
            ..make_char("A", 72.0, 720.0, 7.0, "Helvetica", 12.0)
        },
        RawChar {
            mcid: Some(2),
            ..make_char("B", 79.0, 720.0, 7.0, "Helvetica", 12.0)
        },
    ];

    let spans = group_chars_into_spans(&chars, &dims);
    assert_eq!(spans.len(), 2);
    assert_eq!(spans[0].mcid, Some(1));
    assert_eq!(spans[1].mcid, Some(2));
}

#[test]
fn group_inserts_space_on_gap() {
    let dims = letter_page();
    let chars = vec![
        make_char("H", 72.0, 720.0, 7.0, "Helvetica", 12.0),
        make_char("i", 79.0, 720.0, 4.0, "Helvetica", 12.0),
        make_char("W", 86.0, 720.0, 9.0, "Helvetica", 12.0),
    ];

    let spans = group_chars_into_spans(&chars, &dims);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].content, "Hi W");
}

#[test]
fn char_bboxes_aligned_with_content() {
    let dims = letter_page();
    let chars = vec![
        make_char("H", 72.0, 720.0, 7.0, "Helvetica", 12.0),
        make_char("i", 79.0, 720.0, 4.0, "Helvetica", 12.0),
        make_char("W", 86.0, 720.0, 9.0, "Helvetica", 12.0),
    ];

    let spans = group_chars_into_spans(&chars, &dims);
    assert_eq!(spans.len(), 1);
    let span = &spans[0];
    let content_char_count = span.content.chars().count();
    assert_eq!(
        span.char_bboxes.len(),
        content_char_count,
        "char_bboxes length ({}) must equal content char count ({}) for '{}'",
        span.char_bboxes.len(),
        content_char_count,
        span.content
    );
}

#[test]
fn color_extraction_non_black() {
    let dims = letter_page();
    let chars = vec![RawChar {
        color_r: 1.0,
        color_g: 0.0,
        color_b: 0.0,
        ..make_char("R", 72.0, 720.0, 7.0, "Helvetica", 12.0)
    }];

    let spans = group_chars_into_spans(&chars, &dims);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].color, Some(Color::rgb(255, 0, 0)));
}

#[test]
fn color_none_for_black_text() {
    let dims = letter_page();
    let chars = vec![make_char("B", 72.0, 720.0, 7.0, "Helvetica", 12.0)];

    let spans = group_chars_into_spans(&chars, &dims);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].color, None);
}

#[test]
fn sorting_handles_unordered_chars() {
    let dims = letter_page();
    let chars = vec![
        make_char("B", 79.0, 720.0, 7.0, "Helvetica", 12.0),
        make_char("A", 72.0, 720.0, 7.0, "Helvetica", 12.0),
    ];

    let spans = group_chars_into_spans(&chars, &dims);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].content, "AB");
}

#[test]
fn group_rtl_span_reverses_visual_order() {
    let dims = letter_page();
    let chars = vec![
        make_char("ב", 72.0, 720.0, 7.0, "Helvetica", 12.0),
        make_char("א", 79.0, 720.0, 7.0, "Helvetica", 12.0),
    ];

    let spans = group_chars_into_spans(&chars, &dims);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].content, "אב");
    assert_eq!(spans[0].text_direction, TextDirection::RightToLeft);
    assert_eq!(spans[0].text_direction_origin, ValueOrigin::Reconstructed);
}

#[test]
fn group_vertical_span_marks_top_to_bottom() {
    let dims = letter_page();
    let chars = vec![
        make_rotated_char("縦", 72.0, 720.0, 12.0, "Helvetica", 12.0, 90.0),
        make_rotated_char("書", 72.0, 708.0, 12.0, "Helvetica", 12.0, 90.0),
    ];

    let spans = group_chars_into_spans(&chars, &dims);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].content, "縦書");
    assert_eq!(spans[0].text_direction, TextDirection::TopToBottom);
    assert_eq!(spans[0].text_direction_origin, ValueOrigin::Observed);
}

#[test]
fn skewed_line_preserves_ltr_reading_order() {
    // Simulate a skewed scan: a single visual line of 5 chars where each
    // successive char is placed slightly lower (raw_y decreases as x
    // increases). A naive (y, x) sort would emit the chars in reverse
    // order because ascending-y puts the rightmost char first.
    let dims = letter_page();
    let chars = vec![
        make_char("H", 72.0, 720.0, 7.0, "Helvetica", 12.0),
        make_char("e", 79.5, 719.3, 5.0, "Helvetica", 12.0),
        make_char("l", 85.0, 718.7, 3.0, "Helvetica", 12.0),
        make_char("l", 88.5, 718.0, 3.0, "Helvetica", 12.0),
        make_char("o", 92.0, 717.4, 6.0, "Helvetica", 12.0),
    ];

    let spans = group_chars_into_spans(&chars, &dims);
    let combined: String = spans.iter().map(|s| s.content.as_str()).collect();
    assert_eq!(
        combined,
        "Hello",
        "skewed chars should read left-to-right, got spans: {:?}",
        spans.iter().map(|s| &s.content).collect::<Vec<_>>()
    );
}

#[test]
fn skewed_chars_given_in_reverse_order_still_read_left_to_right() {
    // Same skewed line but chars arrive in reverse (rightmost first): the
    // content-stream order the bug originally manifested on.
    let dims = letter_page();
    let chars = vec![
        make_char("o", 92.0, 717.4, 6.0, "Helvetica", 12.0),
        make_char("l", 88.5, 718.0, 3.0, "Helvetica", 12.0),
        make_char("l", 85.0, 718.7, 3.0, "Helvetica", 12.0),
        make_char("e", 79.5, 719.3, 5.0, "Helvetica", 12.0),
        make_char("H", 72.0, 720.0, 7.0, "Helvetica", 12.0),
    ];

    let spans = group_chars_into_spans(&chars, &dims);
    let combined: String = spans.iter().map(|s| s.content.as_str()).collect();
    assert_eq!(combined, "Hello");
}

#[test]
fn two_distinct_lines_are_ordered_top_to_bottom() {
    // PDF y-up: the "top" line has the larger raw_y. The existing ordering
    // convention emits ascending raw_y first (bottom first), so we preserve
    // that here. Downstream spatial reordering depends on bbox, not on this
    // emission order, but we still want lines to be *separated* cleanly.
    let dims = letter_page();
    let chars = vec![
        make_char("B", 72.0, 700.0, 7.0, "Helvetica", 12.0), // lower line
        make_char("A", 72.0, 720.0, 7.0, "Helvetica", 12.0), // upper line
    ];

    let spans = group_chars_into_spans(&chars, &dims);
    assert_eq!(
        spans.len(),
        2,
        "two separate y-positions should be two spans"
    );
    let texts: Vec<&str> = spans.iter().map(|s| s.content.as_str()).collect();
    assert!(texts.contains(&"A") && texts.contains(&"B"));
}

#[test]
fn infer_document_language_detects_hebrew_and_japanese() {
    assert_eq!(
        infer_document_language_tag(&['ש', 'ל', 'ו', 'ם']),
        Some("he".to_string())
    );
    assert_eq!(
        infer_document_language_tag(&['こ', 'ん', 'に', 'ち', 'は']),
        Some("ja".to_string())
    );
}

// ---------------------------------------------------------------------------
// Diagnostic tests for the skew-tolerant reading-order feature.
// Added to reproduce a reported "incorrect rotation correction" bug.
// ---------------------------------------------------------------------------

/// Two clearly-separated lines should be emitted TOP-FIRST, matching the
/// visual reading order. The existing `two_distinct_lines_are_ordered_top_to_bottom`
/// test only checks that both spans *exist*, not the order, which masks the bug.
#[test]
fn diag_two_lines_actually_emit_top_first() {
    let dims = letter_page();
    let chars = vec![
        make_char("B", 72.0, 700.0, 7.0, "Helvetica", 12.0), // lower line (y-up: smaller raw_y)
        make_char("A", 72.0, 720.0, 7.0, "Helvetica", 12.0), // upper line
    ];

    let spans = group_chars_into_spans(&chars, &dims);
    let texts: Vec<&str> = spans.iter().map(|s| s.content.as_str()).collect();
    assert_eq!(
        texts,
        vec!["A", "B"],
        "expected top-line first, got {:?}",
        texts
    );
}

/// Two skewed horizontal lines close to each other (realistic scan with ~3° tilt
/// and tight leading) should remain SEPARATED, not merge into one cluster that
/// scrambles characters left-to-right across lines.
///
/// Each char has `advance_width == raw_width == 10pt` matching the x-step, so
/// no inter-char word-gap is synthesized (we're testing line clustering, not
/// word splitting).
#[test]
fn diag_two_skewed_lines_dont_merge() {
    let dims = letter_page();
    // Font size 10, realistic leading of ~3pt → baselines 13pt apart.
    // Each line: 5 chars, slight down-right skew (~0.6pt per char).
    let chars = vec![
        // Upper line "HELLO" baseline y=720..717.6 (step -0.6)
        make_char("H", 72.0, 720.0, 10.0, "Helvetica", 10.0),
        make_char("E", 82.0, 719.4, 10.0, "Helvetica", 10.0),
        make_char("L", 92.0, 718.8, 10.0, "Helvetica", 10.0),
        make_char("L", 102.0, 718.2, 10.0, "Helvetica", 10.0),
        make_char("O", 112.0, 717.6, 10.0, "Helvetica", 10.0),
        // Lower line "WORLD" baseline y=707..704.6 (step -0.6)
        make_char("W", 72.0, 707.0, 10.0, "Helvetica", 10.0),
        make_char("O", 82.0, 706.4, 10.0, "Helvetica", 10.0),
        make_char("R", 92.0, 705.8, 10.0, "Helvetica", 10.0),
        make_char("L", 102.0, 705.2, 10.0, "Helvetica", 10.0),
        make_char("D", 112.0, 704.6, 10.0, "Helvetica", 10.0),
    ];

    let spans = group_chars_into_spans(&chars, &dims);
    let combined: String = spans
        .iter()
        .map(|s| s.content.as_str())
        .collect::<Vec<_>>()
        .join("|");
    // After the fix we expect the upper line first (y-up top = larger raw_y),
    // and NEVER any interleaving of chars from the two lines.
    assert_eq!(
        combined,
        "HELLO|WORLD",
        "skewed lines scrambled or ordered wrong: {:?}",
        spans.iter().map(|s| &s.content).collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// Regression tests for the skew-aware line-clustering rewrite
// (industry-standard: median-of-rotation_degrees + projection +
//  pairwise non-expansive clustering). These exercise the worst cases the
// old elastic-range code produced: line-merging under tight leading + skew,
// and bottom-first output. They also pin the axis-aligned equivalence
// and CJK vertical behavior so the fix doesn't regress anything.
// ---------------------------------------------------------------------------

/// Helper: build a char where `rotation_degrees` is also stored so we can
/// simulate a PDF whose text matrix carries the skew per glyph. The position
/// is taken as a baseline origin (origin_x/origin_y) and matches raw_x/raw_y.
fn make_rotated_baseline_char(
    text: &str,
    origin_x: f32,
    origin_y: f32,
    advance: f32,
    font_size: f32,
    rotation_degrees: f32,
) -> RawChar {
    RawChar {
        text: text.chars().next().unwrap_or(' '),
        raw_x: origin_x,
        raw_y: origin_y,
        raw_width: advance,
        raw_height: font_size,
        font_name: "Helvetica".to_string(),
        font_size,
        is_bold: false,
        is_italic: false,
        color_r: 0.0,
        color_g: 0.0,
        color_b: 0.0,
        is_monospace: false,
        origin_x,
        origin_y,
        advance_width: advance,
        rotation_degrees,
        mcid: None,
    }
}

/// Three-line document with 3° skew: worst case for the old elastic-range
/// algorithm. With per-glyph rotation carried by the text matrix (as real PDFs
/// do), the new algorithm should read top-to-bottom without interleaving.
#[test]
fn skewed_3_lines_read_top_to_bottom_with_rotation_signal() {
    let dims = letter_page();
    // Skew = 3°. tan(3°) ≈ 0.0524. Over 100pt x-extent, drift ≈ 5.2pt.
    // Baselines: line A at y=720, line B at y=708, line C at y=696.
    // Each line has 5 chars at x = 72, 82, 92, 102, 112, with y decreasing
    // by 0.524 per char (skew drift).
    let theta_deg: f32 = 3.0;
    let dx_step: f32 = 10.0;
    let dy_step: f32 = dx_step * theta_deg.to_radians().tan();

    let mk_line = |txt: &[&str], base_y: f32| {
        txt.iter()
            .enumerate()
            .map(|(i, t)| {
                make_rotated_baseline_char(
                    t,
                    72.0 + (i as f32) * dx_step,
                    base_y - (i as f32) * dy_step,
                    dx_step,
                    10.0,
                    -theta_deg, // rotation_degrees encodes skew (clockwise)
                )
            })
            .collect::<Vec<_>>()
    };

    let mut chars: Vec<RawChar> = Vec::new();
    chars.extend(mk_line(&["A", "L", "P", "H", "A"], 720.0));
    chars.extend(mk_line(&["B", "E", "T", "A", "!"], 708.0));
    chars.extend(mk_line(&["G", "A", "M", "M", "A"], 696.0));

    let spans = group_chars_into_spans(&chars, &dims);
    let combined: String = spans
        .iter()
        .map(|s| s.content.as_str())
        .collect::<Vec<_>>()
        .join("|");
    assert_eq!(
        combined,
        "ALPHA|BETA!|GAMMA",
        "3-line skewed document should read top-to-bottom: {:?}",
        spans.iter().map(|s| &s.content).collect::<Vec<_>>()
    );
}

/// Tight leading (baselines only 10pt apart at font_size=10) + skew: the
/// classic case that produced "charabia" under the old elastic-range logic
/// because the first line's running max_y eventually overlapped the second
/// line's min_y.
#[test]
fn tight_leading_with_skew_does_not_merge_lines() {
    let dims = letter_page();
    let theta_deg: f32 = 2.5;
    let dy_step: f32 = 10.0 * theta_deg.to_radians().tan();

    let mut chars = Vec::new();
    // 6 chars per line; tight leading (10pt apart).
    for (base_y, text) in [(720.0_f32, "UPPERS"), (710.0, "LOWERS")] {
        for (i, c) in text.chars().enumerate() {
            chars.push(make_rotated_baseline_char(
                &c.to_string(),
                72.0 + (i as f32) * 10.0,
                base_y - (i as f32) * dy_step,
                10.0,
                10.0,
                -theta_deg,
            ));
        }
    }

    let spans = group_chars_into_spans(&chars, &dims);
    let combined: String = spans
        .iter()
        .map(|s| s.content.as_str())
        .collect::<Vec<_>>()
        .join("|");
    assert_eq!(
        combined,
        "UPPERS|LOWERS",
        "tight-leading skewed lines must not merge: {:?}",
        spans.iter().map(|s| &s.content).collect::<Vec<_>>()
    );
}

/// Axis-aligned (θ=0) pages must be byte-identical to a simple "sort by
/// -raw_y, then raw_x" baseline. This pins the non-regression on the common
/// case that represents 99 % of real-world PDFs.
#[test]
fn axis_aligned_multi_line_emits_top_to_bottom_left_to_right() {
    let dims = letter_page();
    let mut chars = Vec::new();
    for (base_y, text) in [(720.0_f32, "FIRST"), (708.0, "SECON"), (696.0, "THIRD")] {
        for (i, c) in text.chars().enumerate() {
            chars.push(make_char(
                &c.to_string(),
                72.0 + (i as f32) * 10.0,
                base_y,
                10.0,
                "Helvetica",
                10.0,
            ));
        }
    }

    let spans = group_chars_into_spans(&chars, &dims);
    let combined: String = spans
        .iter()
        .map(|s| s.content.as_str())
        .collect::<Vec<_>>()
        .join("|");
    assert_eq!(combined, "FIRST|SECON|THIRD");
}

/// A single skewed line is stable: no false line-break even with 5° skew
/// across 10 chars. `skewed_line_preserves_ltr_reading_order` covers the 5-char
/// case with tiny drift; this one pushes it to a realistic scan angle.
#[test]
fn single_line_5deg_skew_stays_one_line() {
    let dims = letter_page();
    let theta_deg: f32 = 5.0;
    let dy_step: f32 = 10.0 * theta_deg.to_radians().tan(); // ≈ 0.875pt/char

    let mut chars = Vec::new();
    for (i, c) in "SKEWEDTEXT".chars().enumerate() {
        chars.push(make_rotated_baseline_char(
            &c.to_string(),
            72.0 + (i as f32) * 10.0,
            720.0 - (i as f32) * dy_step,
            10.0,
            10.0,
            -theta_deg,
        ));
    }

    let spans = group_chars_into_spans(&chars, &dims);
    let combined: String = spans.iter().map(|s| s.content.as_str()).collect();
    assert_eq!(
        combined,
        "SKEWEDTEXT",
        "single skewed line must stay unbroken: {:?}",
        spans.iter().map(|s| &s.content).collect::<Vec<_>>()
    );
}

/// Subscripts and superscripts sharing a line with body text must stay on
/// that line thanks to the tolerance being scaled by `max(font_size)`.
/// E.g. "x² + y" where '²' is at smaller font_size and sits slightly above
/// the baseline.
#[test]
fn subscript_superscript_joins_body_line() {
    let dims = letter_page();
    // Body chars at baseline y=720 font_size=10, superscript at y=723 font_size=6.
    let chars = vec![
        make_char("x", 72.0, 720.0, 6.0, "Helvetica", 10.0),
        make_char("2", 78.0, 723.0, 3.6, "Helvetica", 6.0), // superscript
        make_char(" ", 81.6, 720.0, 3.0, "Helvetica", 10.0),
        make_char("+", 84.6, 720.0, 6.0, "Helvetica", 10.0),
        make_char(" ", 90.6, 720.0, 3.0, "Helvetica", 10.0),
        make_char("y", 93.6, 720.0, 6.0, "Helvetica", 10.0),
    ];

    let spans = group_chars_into_spans(&chars, &dims);
    // Superscript has different font_size → span-break logic will split the
    // span into separate TextSpans, but all must be present in one logical
    // line (no char scrambling, no "2" emitted on its own line before "x").
    let all: String = spans
        .iter()
        .map(|s| s.content.as_str())
        .collect::<Vec<_>>()
        .join("");
    // The critical check: "x" must precede "2" must precede "+" must precede "y".
    let idx_x = all.find('x');
    let idx_2 = all.find('2');
    let idx_plus = all.find('+');
    let idx_y = all.find('y');
    assert!(
        idx_x < idx_2 && idx_2 < idx_plus && idx_plus < idx_y,
        "expected x..2..+..y order, got {:?}",
        all
    );
}

/// An inconsistent rotation signal (mixed stamps/watermarks with strong
/// rotation inside otherwise horizontal text) must fall back to θ=0 instead
/// of warping the whole page. `estimate_line_angle_radians` uses median-abs-
/// deviation to detect this.
#[test]
fn inconsistent_rotation_falls_back_to_zero_theta() {
    let dims = letter_page();
    // 4 horizontal chars + 1 strongly-rotated stamp char. MAD should be high.
    let chars = vec![
        make_rotated_baseline_char("A", 72.0, 720.0, 10.0, 10.0, 0.0),
        make_rotated_baseline_char("B", 82.0, 720.0, 10.0, 10.0, 0.0),
        make_rotated_baseline_char("C", 92.0, 720.0, 10.0, 10.0, 0.0),
        make_rotated_baseline_char("D", 102.0, 720.0, 10.0, 10.0, 14.0), // outlier
    ];
    let spans = group_chars_into_spans(&chars, &dims);
    let all: String = spans
        .iter()
        .map(|s| s.content.as_str())
        .collect::<Vec<_>>()
        .join("");
    // With θ=0 fallback, the horizontal chars still cluster on y=720 and read
    // left-to-right. The rotated outlier joins because its raw_y is identical.
    // Main property: no scrambling, A..B..C..D order preserved.
    let pos: Vec<usize> = ['A', 'B', 'C', 'D']
        .iter()
        .map(|c| all.find(*c).expect("missing char"))
        .collect();
    assert!(
        pos[0] < pos[1] && pos[1] < pos[2] && pos[2] < pos[3],
        "chars out of order under rotation fallback: {:?}",
        all
    );
}

/// CJK vertical (rotation=90°) columns are routed to the vertical clusterer.
/// A right-to-left document order (rightmost column first) with two columns
/// must not get mangled by the horizontal logic.
#[test]
fn vertical_text_two_columns_stay_separated() {
    let dims = letter_page();
    // Two CJK-style vertical columns at x=100 and x=150, with tight 1.0×
    // leading (y-step = font_size) so that bbox-adjacent glyphs do not trip
    // the intra-span vertical gap threshold.
    //
    // Column 2 is provided in reverse to also exercise the pre-sort.
    let chars = vec![
        // Right column (fed first, appears second left→right)
        make_rotated_char("一", 150.0, 720.0, 12.0, "Helvetica", 12.0, 90.0),
        make_rotated_char("二", 150.0, 708.0, 12.0, "Helvetica", 12.0, 90.0),
        make_rotated_char("三", 150.0, 696.0, 12.0, "Helvetica", 12.0, 90.0),
        // Left column
        make_rotated_char("A", 100.0, 720.0, 12.0, "Helvetica", 12.0, 90.0),
        make_rotated_char("B", 100.0, 708.0, 12.0, "Helvetica", 12.0, 90.0),
        make_rotated_char("C", 100.0, 696.0, 12.0, "Helvetica", 12.0, 90.0),
    ];
    let spans = group_chars_into_spans(&chars, &dims);
    // order_vertical_chars_by_column sorts columns by ascending X (left→right),
    // each column internally top-to-bottom. Expect "ABC" then "一二三", with
    // no interleaving between the two columns.
    assert_eq!(spans.len(), 2, "expected 2 vertical spans, got {:?}", spans);
    assert_eq!(spans[0].content, "ABC");
    assert_eq!(spans[1].content, "一二三");
}
