use pdf_oxide::layout::{FontWeight, TextChar};

use crate::formats::pdf::pages::{clean_font_name, is_bold_font, is_italic_font};

pub(super) struct RawWord {
    pub(super) text: String,
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) w: f32,
    pub(super) h: f32,
    pub(super) is_bold: bool,
    pub(super) is_italic: bool,
    pub(super) link: Option<String>,
    /// `true` when the bucket tokenizer trimmed a LEADING space glyph
    /// from this word — see the twin field on
    /// `crate::output::spatial::words::RawWord` for the directional
    /// rationale behind splitting leading/trailing separately.
    pub(super) absorbed_leading: bool,
    /// `true` when the bucket tokenizer trimmed a TRAILING space glyph
    /// from this word — see the twin field on
    /// `crate::output::spatial::words::RawWord` for the directional
    /// rationale behind splitting leading/trailing separately.
    pub(super) absorbed_trailing: bool,
}

fn is_bold_char(ch: &TextChar) -> bool {
    matches!(
        ch.font_weight,
        FontWeight::Bold | FontWeight::ExtraBold | FontWeight::Black | FontWeight::SemiBold
    ) || is_bold_font(&ch.font_name)
}

fn is_italic_char(ch: &TextChar) -> bool {
    ch.is_italic || is_italic_font(&ch.font_name)
}

pub(super) fn group_chars_into_words(chars: &[TextChar]) -> Vec<RawWord> {
    if chars.is_empty() {
        return Vec::new();
    }

    // Sort primary Y-desc, secondary base-font-name-asc, tertiary X-asc.
    // Font as the secondary key groups chars of a single text run
    // together when two runs with different fonts overlap in X on the
    // same baseline — the common case being a left-cell label whose end
    // visually spills into a right-cell value drawn in a different face
    // (e.g. bold label `homologation` + regular date `2025-11-17` in a
    // monograph ID table). Without this, the X-asc secondary key alone
    // interleaves glyphs character-by-character and the tokenizer sees
    // one corrupted word like `homologa2t0io2n5-11-17`. With it, each
    // font bucket is X-monotonic, and the transition between buckets
    // produces a large backward X gap that the existing break threshold
    // catches cleanly, yielding `d'homologation` and `2025-11-17` as
    // two distinct words placed at their correct X positions.
    //
    // We compare the BASE font name (subset prefix stripped, per ISO
    // 32000-2 §9.6.4) rather than the raw pdf_oxide `font_name`. PDFs
    // routinely split a single logical face into multiple subset fonts
    // with distinct 6-letter prefixes (`BCDEEE+Calibri-Bold` and
    // `BCDFEE+Calibri-Bold` on `Cours CR300v2`); a raw-name sort puts
    // them in different buckets on the same baseline, the tokenizer
    // splits a natural word in two, and the grid renderer paints each
    // half at its own PDF X — which produces visible double-spaces
    // wherever the subset boundary falls inside a word. The mirror
    // markdown pipeline keeps the same fix in lockstep with
    // `spatial::words` so both outputs see the same tokenization.
    //
    // Spaces are sorted using the EFFECTIVE font (the nearest preceding
    // non-space char's font on the same baseline), not their own.
    // PDF justification engines routinely emit padding spaces in a
    // dedicated font (`R8604` on ansys_13_command_reference.pdf): a
    // raw-name sort orphans those spaces into a separate bucket where
    // they trim to empty and disappear, leaving the surrounding text
    // bucket with `.` and `T` adjacent and fusing into `provisions.The`.
    // Re-homing the space into the predecessor's bucket keeps it as an
    // internal-word character and the boundary is preserved. See
    // `spatial::words::group_chars_into_words` for the longer rationale.
    let own_font: Vec<String> = chars
        .iter()
        .map(|ch| clean_font_name(&ch.font_name))
        .collect();
    let effective_font: Vec<&str> = {
        let mut result: Vec<&str> = Vec::with_capacity(chars.len());
        let mut last_nonspace: Option<(usize, f32, f32)> = None; // (idx, y, h)
        for (i, ch) in chars.iter().enumerate() {
            let h = ch.bbox.height.max(1.0);
            if ch.char == ' ' {
                match last_nonspace {
                    Some((j, y, jh)) if (ch.bbox.y - y).abs() < jh.max(h) * 0.5 => {
                        result.push(&own_font[j]);
                    }
                    _ => result.push(&own_font[i]),
                }
            } else {
                result.push(&own_font[i]);
                last_nonspace = Some((i, ch.bbox.y, h));
            }
        }
        result
    };

    let mut sorted_idx: Vec<usize> = (0..chars.len()).collect();
    sorted_idx.sort_by(|&a, &b| {
        chars[b]
            .bbox
            .y
            .total_cmp(&chars[a].bbox.y)
            .then_with(|| effective_font[a].cmp(effective_font[b]))
            .then_with(|| chars[a].bbox.x.total_cmp(&chars[b].bbox.x))
    });
    let sorted: Vec<&TextChar> = sorted_idx.iter().map(|&i| &chars[i]).collect();

    let mut words: Vec<RawWord> = Vec::with_capacity(chars.len() / 5 + 1);
    let mut current_text = String::new();
    let mut current_x: f32 = 0.0;
    let mut current_y: f32 = 0.0;
    let mut current_end: f32 = 0.0;
    let mut current_h: f32 = 0.0;
    let mut bold_count: usize = 0;
    let mut italic_count: usize = 0;
    let mut char_count: usize = 0;

    for (i, ch) in sorted.iter().enumerate() {
        let c = ch.char;
        if c.is_control() && c != ' ' {
            continue;
        }

        if i == 0 {
            current_text.push(c);
            current_x = ch.bbox.x;
            current_y = ch.bbox.y;
            current_end = ch.bbox.x + ch.bbox.width;
            current_h = ch.bbox.height;
            char_count = 1;
            bold_count = usize::from(is_bold_char(ch));
            italic_count = usize::from(is_italic_char(ch));
            continue;
        }

        let y_diff = (ch.bbox.y - current_y).abs();
        let same_line = y_diff < current_h.max(ch.bbox.height) * 0.5;
        let gap = ch.bbox.x - current_end;
        // Backward-jump threshold: strictly tighter than a full line height
        // so overlapping text runs on the same baseline (e.g. `Due` drawn
        // over `Montant` in a bilingual table header — contract.pdf p2)
        // separate into distinct words. Kerning only produces backward
        // gaps of ~-0.5pt, well inside this tolerance. Loose threshold
        // (`< -current_h`) would miss these: observed backward jumps at
        // run boundaries are -2 to -5pt, which are >30% of the 8pt font
        // height but under one full cell.
        let is_break = !same_line || gap > current_h * 0.3 || gap < -current_h * 0.3;

        if is_break {
            let trimmed = current_text.trim();
            if !trimmed.is_empty() {
                words.push(RawWord {
                    text: trimmed.to_string(),
                    x: current_x,
                    y: current_y,
                    w: current_end - current_x,
                    h: current_h,
                    is_bold: char_count > 0 && bold_count * 2 >= char_count,
                    is_italic: char_count > 0 && italic_count * 2 >= char_count,
                    link: None,
                    absorbed_leading: current_text.starts_with(' '),
                    absorbed_trailing: current_text.ends_with(' '),
                });
            }
            current_text.clear();
            current_text.push(c);
            current_x = ch.bbox.x;
            current_y = ch.bbox.y;
            current_end = ch.bbox.x + ch.bbox.width;
            current_h = ch.bbox.height;
            char_count = 1;
            bold_count = usize::from(is_bold_char(ch));
            italic_count = usize::from(is_italic_char(ch));
        } else {
            current_text.push(c);
            current_end = (ch.bbox.x + ch.bbox.width).max(current_end);
            current_h = current_h.max(ch.bbox.height);
            char_count += 1;
            bold_count += usize::from(is_bold_char(ch));
            italic_count += usize::from(is_italic_char(ch));
        }
    }

    let trimmed = current_text.trim();
    if !trimmed.is_empty() {
        words.push(RawWord {
            text: trimmed.to_string(),
            x: current_x,
            y: current_y,
            w: current_end - current_x,
            h: current_h,
            is_bold: char_count > 0 && bold_count * 2 >= char_count,
            is_italic: char_count > 0 && italic_count * 2 >= char_count,
            link: None,
            absorbed_leading: current_text.starts_with(' '),
            absorbed_trailing: current_text.ends_with(' '),
        });
    }

    words
}

/// Re-merge same-line `PlacedWord`s split only by a font-bucket boundary.
///
/// Mirror of the spatial-path `merge_cross_font_adjacent_placed` — see
/// `crate::output::spatial::words` for the structural rationale.
///
/// Runs after `compute_target_cols` has calibrated the grid against
/// per-fragment ratios. Merging before that step would inflate `chars/width`
/// by the synthetic space we add and drift otherwise-pristine layouts
/// (the weird_invoice shift we caught in testing).
///
/// The markdown path tracks per-word `is_bold`/`is_italic`, which the
/// font-bucket tokenizer computed by majority over the glyphs it saw.
/// When merging a regular and bold fragment into a single word we lose
/// the ability to emit per-fragment `**bold**` markers — but that is
/// already the behavior for words that are partially bold within a
/// single font bucket, and mainstream PDF extractors likewise collapse
/// to per-word emphasis for plain-text and markdown output. We preserve
/// the majority flag across the merged text so a `Très **important**`
/// sequence still tilts the merged word toward bold when most glyphs
/// were bold, without introducing nested emphasis that the grid
/// renderer cannot represent anyway.
pub(super) fn merge_cross_font_adjacent_placed(
    mut placed: Vec<super::placement::PlacedWord>,
    target_cols: usize,
) -> Vec<super::placement::PlacedWord> {
    if placed.len() < 2 {
        return placed;
    }
    // Placed coordinates are normalized with y inverted
    // (`1.0 - raw_y/ch`), so reading order is y-asc, x-asc.
    placed.sort_by(|a, b| a.y.total_cmp(&b.y).then_with(|| a.x.total_cmp(&b.x)));

    let cols_f = (target_cols.saturating_sub(1) as f32).max(1.0);
    // Tuple carries (word, bold-count, italic-count, total-char-count,
    // running_chars). `char_count` tracks original chars only (drives
    // bold/italic majority); `running_chars` tracks rendered length
    // including padding (drives grid-column gap preservation).
    let mut merged: Vec<(super::placement::PlacedWord, usize, usize, usize, usize)> =
        Vec::with_capacity(placed.len());
    for p in placed {
        let p_chars = p.text.chars().count();
        let p_bold = if p.is_bold { p_chars } else { 0 };
        let p_italic = if p.is_italic { p_chars } else { 0 };
        if let Some((last, b, it, char_count, running_chars)) = merged.last_mut() {
            // Per-word `h` (normalized column-units) reproduces the
            // within-bucket tokenizer's scale exactly — see the twin
            // comment in `spatial::words::merge_cross_font_adjacent_placed`
            // for the full reasoning.
            let h = last.h.max(p.h);
            let y_tol = h * 0.5;
            let gap_ceiling = h * 0.3;
            let gap_floor = -h * 0.3;
            let same_line = (last.y - p.y).abs() < y_tol;
            let gap = p.x - (last.x + last.w);
            if same_line && gap >= gap_floor && gap <= gap_ceiling {
                // See the twin comment in
                // `spatial::words::merge_cross_font_adjacent_placed`
                // for the full rationale and the directional-split
                // motivation: only a trailing space on `last` OR a
                // leading space on `p` is a sentence-flow signal AT
                // THIS boundary. A leading space absorbed from an
                // unrelated bullet-prefix run (e.g. F55 space between
                // `•` and bold `French` on the CV) says nothing about
                // the right-hand edge and must not collapse to a
                // single space when the actual boundary is pure
                // kerning against a column separator.
                let sentence_flow = last.absorbed_trailing || p.absorbed_leading;
                let pad_len = if sentence_flow {
                    1
                } else {
                    let last_start_col = (last.x * cols_f).round() as i64;
                    let p_start_col = (p.x * cols_f).round() as i64;
                    (p_start_col - last_start_col - *running_chars as i64).max(1) as usize
                };
                for _ in 0..pad_len {
                    last.text.push(' ');
                }
                last.text.push_str(&p.text);
                last.w = (p.x + p.w) - last.x;
                last.h = last.h.max(p.h);
                // After merge, the combined token inherits `p`'s
                // trailing edge — that's the edge the NEXT merge
                // candidate will sit against. The leading edge stays
                // with the left fragment that remained at `last.x`.
                last.absorbed_trailing = p.absorbed_trailing;
                if last.link.is_none() {
                    last.link = p.link;
                }
                *running_chars += pad_len + p_chars;
                // Running bold/italic counts contribute to the
                // majority over the original chars only; synthetic
                // padding is typographic, not content, and is
                // excluded from `char_count`.
                *b += p_bold;
                *it += p_italic;
                *char_count += p_chars;
                last.is_bold = *b * 2 >= *char_count;
                last.is_italic = *it * 2 >= *char_count;
                continue;
            }
        }
        merged.push((p, p_bold, p_italic, p_chars, p_chars));
    }
    merged.into_iter().map(|(p, _, _, _, _)| p).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdf_oxide::geometry::Rect;

    /// Build a TextChar for tokenizer tests. `weight` controls whether
    /// the char looks bold to the grouper (via FontWeight::Bold), and
    /// `font` name is what drives the secondary sort key.
    fn mk_char(c: char, x: f32, y: f32, w: f32, h: f32, font: &str, bold: bool) -> TextChar {
        TextChar {
            char: c,
            bbox: Rect {
                x,
                y,
                width: w,
                height: h,
            },
            font_name: font.to_string(),
            font_weight: if bold {
                FontWeight::Bold
            } else {
                FontWeight::Normal
            },
            advance_width: w,
            ..TextChar::default()
        }
    }

    /// Regression test pinning the font-aware secondary sort.
    ///
    /// Reproduces the monograph.pdf p1 "Approval date / Date
    /// d'homologation 2025-11-17" collision where the bold label's tail
    /// `t`, `i`, `o`, `n` is rendered in the same X range as the regular
    /// date's `2`, `0`, `2`, `5`. An X-only sort would interleave them
    /// into a single garbled token. Font-secondary sort separates the
    /// runs, and the additional bold/regular run also carries bold vs
    /// non-bold flags into the resulting words — which drives markdown
    /// emphasis rendering downstream.
    #[test]
    fn overlapping_cells_with_different_fonts_yield_separate_words() {
        let bold = "AAAAAA+DejaVuSans-Bold";
        let reg = "AAAAAA+DejaVuSans";
        let y = 485.0;
        let h = 8.0;
        let chars = vec![
            mk_char('t', 10.0, y, 1.8, h, bold, true),
            mk_char('2', 11.0, y, 1.8, h, reg, false),
            mk_char('i', 12.0, y, 1.8, h, bold, true),
            mk_char('0', 13.0, y, 1.8, h, reg, false),
            mk_char('o', 14.0, y, 1.8, h, bold, true),
            mk_char('2', 15.0, y, 1.8, h, reg, false),
            mk_char('n', 16.0, y, 1.8, h, bold, true),
            mk_char('5', 17.0, y, 1.8, h, reg, false),
        ];

        let words = group_chars_into_words(&chars);
        let mut out: Vec<(&str, bool)> =
            words.iter().map(|w| (w.text.as_str(), w.is_bold)).collect();
        out.sort_unstable_by_key(|(t, _)| *t);

        // Runs emerge as two distinct tokens, each carrying its own
        // bold flag (the tail of the label is bold, the date is not).
        // Sort-before-compare: emission order depends on font-name sort
        // key, but that's incidental — what matters is no interleaving.
        assert_eq!(
            out,
            vec![("2025", false), ("tion", true)],
            "font-aware sort must separate runs and preserve per-run bold flag",
        );
        assert_eq!(words.len(), 2);
    }

    /// Sanity: a single monotonic run in one font collapses to one word.
    #[test]
    fn single_font_run_stays_one_word() {
        let y = 100.0;
        let h = 8.0;
        let chars: Vec<TextChar> = "hello"
            .chars()
            .enumerate()
            .map(|(i, c)| mk_char(c, 10.0 + i as f32 * 5.0, y, 5.0, h, "Helvetica", false))
            .collect();

        let words = group_chars_into_words(&chars);
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "hello");
        assert!(!words[0].is_bold);
    }
}
