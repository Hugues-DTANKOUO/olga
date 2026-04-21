pub(super) struct RawWord {
    pub(super) text: String,
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) w: f32,
    pub(super) h: f32,
    pub(super) link: Option<String>,
    /// `true` when the bucket tokenizer saw — and trimmed — a LEADING
    /// space glyph (the word started with a PDF space that was merged
    /// into this token from the same font bucket). Separating leading
    /// from trailing matters: only the space at the *actual boundary*
    /// between two merge candidates signals sentence flow. A word whose
    /// leading space came from a bullet-prefix run (e.g. the F55 space
    /// glyph between `•` and `French` on the CV) carries no sentence-
    /// flow information across its trailing edge.
    pub(super) absorbed_leading: bool,
    /// `true` when the bucket tokenizer saw — and trimmed — a TRAILING
    /// space glyph from this word. This is the signal the cross-font
    /// merger uses on the LEFT fragment to decide whether the boundary
    /// with the next token is sentence flow (collapse to one literal
    /// space) or structural (preserve the grid-column alignment).
    pub(super) absorbed_trailing: bool,
}

pub(super) fn group_chars_into_words(chars: &[pdf_oxide::layout::TextChar]) -> Vec<RawWord> {
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
    // routinely split a single logical face into several subset fonts
    // with distinct 6-letter prefixes (`BCDEEE+Calibri-Bold` and
    // `BCDFEE+Calibri-Bold` on `Cours CR300v2`): a raw-name sort puts
    // them in different buckets on the same baseline, which breaks
    // natural words in two (`Pour`/`l'examen`) and the grid then paints
    // each placed word at its own PDF X, yielding visible double-spaces
    // wherever the split falls inside a word. Sorting on the logical
    // face keeps the run contiguous so the tokenizer walks it normally.
    //
    // The font of a SPACE glyph, however, is an incidental PDF-encoding
    // artifact — professionally typeset documents routinely ship a tiny
    // dedicated font (e.g. `R8604` on ansys_13_command_reference.pdf)
    // used solely for justification-pad glyphs. If we sort a lone-space
    // glyph by its raw font name, it lands in an otherwise-empty bucket
    // where the final `.trim()` discards it, and the surrounding text
    // bucket is left with `.` and `T` as X-adjacent chars (gap below the
    // break threshold), producing a fused `provisions.The` run.
    //
    // We therefore compute an EFFECTIVE sort-font per char: for non-
    // spaces it's the char's own clean font name; for spaces it borrows
    // the nearest preceding non-space char's font on the same baseline.
    // This re-homes padding spaces into the bucket of the run they
    // semantically separate, where the tokenizer keeps them as internal
    // whitespace and the `.`/`T` boundary is properly punctuated.
    let own_font: Vec<String> = chars
        .iter()
        .map(|ch| crate::formats::pdf::pages::clean_font_name(&ch.font_name))
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
    let sorted: Vec<&pdf_oxide::layout::TextChar> = sorted_idx.iter().map(|&i| &chars[i]).collect();

    let mut words: Vec<RawWord> = Vec::with_capacity(chars.len() / 5 + 1);
    let mut current_text = String::new();
    let mut current_x: f32 = 0.0;
    let mut current_y: f32 = 0.0;
    let mut current_end: f32 = 0.0;
    let mut current_h: f32 = 0.0;

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
        } else {
            current_text.push(c);
            current_end = (ch.bbox.x + ch.bbox.width).max(current_end);
            current_h = current_h.max(ch.bbox.height);
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
            link: None,
            absorbed_leading: current_text.starts_with(' '),
            absorbed_trailing: current_text.ends_with(' '),
        });
    }

    words
}

/// Re-merge same-line `RawWord`s that were only split because they landed in
/// different font buckets.
///
/// The `(Y, font, X)` sort in [`group_chars_into_words`] is required to keep
/// interleaved runs separable (monograph.pdf's bold label tail drawn at the
/// same X as a regular date — see that function's doc). Its side effect on a
/// *normal* bold→regular→bold line (e.g. Cours CR300 — mixed `Calibri` /
/// `Calibri-Bold` cells) is that the grid paints each fragment at its own
/// PDF X in monospace, so proportional widths leak out as visible
/// double-spaces between fragments.
///
/// After bucket tokenization we know two things:
/// 1. Within a bucket, words are already correctly delimited — the
///    tokenizer broke on any `gap > h * 0.3` (roughly "bigger than one
///    space"), so fragments that *stayed* inside a bucket represent
///    consecutive glyphs separated by at most one natural space.
/// 2. Across buckets, fragments that would have been one word under a
///    font-agnostic tokenizer have the same geometric property: the
///    forward gap between them is under `h * 0.3`.
///
/// Merging on that same threshold therefore reunites cross-font fragments
/// without ever coalescing table columns (large forward gap by construction)
/// nor interleaved-cell runs (backward overlap — the gap is deeply negative
/// and falls outside the merge band).
///
/// One literal space is inserted between the fragments, regardless of
/// whether one already carried a trailing/leading space — "one glyph-sized
/// gap ↔ one space" is the invariant we uphold, consistent with the
/// normalization used by mainstream PDF text extractors (pdfminer.six,
/// pdfplumber).
///
/// This step runs *after* [`compute_target_cols`] has calibrated the grid
/// against per-fragment chars-per-width ratios; merging before that step
/// would inflate those ratios by the synthetic space we add, which widens
/// the grid by a few percent and drifts otherwise-pristine layouts (the
/// weird_invoice regression we caught in testing).
pub(super) fn merge_cross_font_adjacent_placed(
    mut placed: Vec<super::placement::PlacedText>,
    target_cols: usize,
) -> Vec<super::placement::PlacedText> {
    if placed.len() < 2 {
        return placed;
    }
    // Placed coordinates are in normalized `[0, 1]` space where `y` was
    // inverted (`1.0 - raw_y/ch`), so reading order is y-asc, x-asc.
    placed.sort_by(|a, b| a.y.total_cmp(&b.y).then_with(|| a.x.total_cmp(&b.x)));

    let cols_f = (target_cols.saturating_sub(1) as f32).max(1.0);
    // Tuple carries (word, running_chars). `running_chars` tracks the
    // current rendered length of the merged token including any
    // previously-inserted synthetic padding, so grid-column gap
    // preservation works correctly across chained merges.
    let mut merged: Vec<(super::placement::PlacedText, usize)> = Vec::with_capacity(placed.len());
    for p in placed {
        let p_chars = p.text.chars().count();
        if let Some((last, running_chars)) = merged.last_mut() {
            // Per-word `h` (normalized into column-units inside
            // `build_placed_texts`) gives us the SAME geometric scale
            // the within-bucket tokenizer uses for its `gap > h * 0.3`
            // break check. Taking the max of the two fragments' heights
            // avoids edge cases where one of the fragments shrunk
            // (e.g. a subscript glyph): we want the check to track the
            // "intended" line height of the run.
            let h = last.h.max(p.h);
            // Same-line tolerance: 0.5 * h is the convention shared
            // with the within-bucket tokenizer (`y_diff < h * 0.5`).
            // Fragments on visibly distinct lines therefore always
            // fail this check — no merge across line wraps.
            let y_tol = h * 0.5;
            let same_line = (last.y - p.y).abs() < y_tol;
            // Forward-merge gap ceiling: 0.3 * h, mirroring the
            // tokenizer's break threshold exactly. A gap larger than
            // this would already have been a word-break inside a
            // single bucket, so it must remain a word-break across
            // buckets too — which is what keeps table columns,
            // tab-aligned cells, and any structural whitespace from
            // ever being collapsed into a single token.
            let gap_ceiling = h * 0.3;
            // Allow a tiny backward overlap (kerning, italic carry)
            // but no more — the `< -h * 0.3` floor in the tokenizer
            // is what catches the "interleaved bilingual cells" case.
            // The merger uses the SAME floor on the negative side.
            let gap_floor = -h * 0.3;
            let gap = p.x - (last.x + last.w);
            if same_line && gap >= gap_floor && gap <= gap_ceiling {
                // Two cross-font fragments on the same baseline, within
                // a kerning-scale gap. The pad we insert depends on
                // whether the tokenizer absorbed an explicit space
                // glyph *at the boundary between these two fragments* —
                // the only signal that separates the structurally
                // distinct cases that otherwise look identical:
                //
                // * Sentence flow (`last.absorbed_trailing` OR
                //   `p.absorbed_leading`). A real PDF space glyph sat
                //   between these two fragments and was merged into
                //   one of them by the tokenizer. Cours CR300
                //   `Votre ` (Calibri-regular, trailing space) →
                //   `examen final` (Calibri-Bold). Without the merge
                //   the bold run lands at its own PDF column, 1–2 cols
                //   past the end of the regular word in the monospace
                //   grid because bold glyphs render proportionally
                //   wider than the regular face the grid is calibrated
                //   against. Collapsing to a single literal space
                //   removes the artifact, following the same
                //   `word_margin`-style convention pdfminer.six uses.
                //
                // * Structural adjacency (neither flag). The boundary
                //   is pure kerning — e.g. a CV's bold country name →
                //   regular `:` where the colon's `x` lies inside the
                //   monospace expansion of the bold word. Forcing a
                //   pad of 1 would ATTACH the colon to the bold word,
                //   losing the column the regular face naturally
                //   places it in. We instead emit `p_start_col -
                //   last_start_col - running_chars` spaces — the gap
                //   that would have appeared if both words had been
                //   placed independently on the grid — preserving the
                //   alignment the PDF author intended.
                //
                // Directionality matters: only a trailing space on
                // `last` OR a leading space on `p` is a real sentence-
                // flow signal at THIS boundary. A word whose leading
                // space came from a bullet-prefix run (e.g. the F55
                // space glyph between `•` and bold `French` on the CV)
                // has `absorbed_leading=true`, but that carries no
                // information about its trailing edge. Collapsing the
                // `absorbed_space` flag to a single bool lost this
                // distinction and produced `French : Native` where
                // the reference (and PDF proportional layout) calls
                // for `French  : Native`.
                //
                // The `.max(1)` floor mirrors `col_resolve`'s
                // one-column invariant: even fragments that would
                // otherwise visually touch get one separator
                // character so the merged token never reads as a
                // run-on word.
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
                // After merge, the merged token inherits the right-
                // fragment's trailing edge — the next boundary is the
                // one `p` presented on its right. The leading edge
                // stays with the left fragment (which remained at
                // `last.x`).
                last.absorbed_trailing = p.absorbed_trailing;
                if last.link.is_none() {
                    last.link = p.link;
                }
                *running_chars += pad_len + p_chars;
                continue;
            }
        }
        merged.push((p, p_chars));
    }
    merged.into_iter().map(|(p, _)| p).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdf_oxide::geometry::Rect;
    use pdf_oxide::layout::TextChar;

    /// Build a TextChar with minimal fields for tokenizer tests. Advance
    /// width is set equal to glyph width so forward gaps match raw X diffs.
    fn mk_char(c: char, x: f32, y: f32, w: f32, h: f32, font: &str) -> TextChar {
        TextChar {
            char: c,
            bbox: Rect {
                x,
                y,
                width: w,
                height: h,
            },
            font_name: font.to_string(),
            advance_width: w,
            ..TextChar::default()
        }
    }

    /// Regression test pinning the font-aware secondary sort.
    ///
    /// Reproduces the monograph.pdf p1 "Approval date / Date
    /// d'homologation 2025-11-17" collision where the bold label's tail
    /// `t`, `i`, `o`, `n` is rendered in the same X range as the regular
    /// date's `2`, `0`, `2`, `5`. A naive X-only sort interleaves them
    /// character-by-character into a single garbled token (`tion` and
    /// `2025` cannot be recovered). With (Y, font, X) sort, each font
    /// run is X-monotonic and the backward X jump at the run boundary
    /// triggers a word break — yielding two clean tokens.
    #[test]
    fn overlapping_cells_with_different_fonts_yield_separate_words() {
        let bold = "AAAAAA+DejaVuSans-Bold";
        let reg = "AAAAAA+DejaVuSans";
        let y = 485.0;
        let h = 8.0;
        // Bold tail of "homologation": t,i,o,n at increasing X.
        // Regular date "2025": 2,0,2,5 interleaved in the same X range.
        // After X-only sort the order would be t,2,i,0,o,2,n,5.
        let chars = vec![
            mk_char('t', 10.0, y, 1.8, h, bold),
            mk_char('2', 11.0, y, 1.8, h, reg),
            mk_char('i', 12.0, y, 1.8, h, bold),
            mk_char('0', 13.0, y, 1.8, h, reg),
            mk_char('o', 14.0, y, 1.8, h, bold),
            mk_char('2', 15.0, y, 1.8, h, reg),
            mk_char('n', 16.0, y, 1.8, h, bold),
            mk_char('5', 17.0, y, 1.8, h, reg),
        ];

        let words = group_chars_into_words(&chars);
        let mut texts: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();
        texts.sort_unstable();

        // The two runs must emerge as distinct clean tokens. We sort
        // before comparing because their emission order depends on the
        // secondary sort key (font name) — what matters for correctness
        // is that they are NOT interleaved, not which comes first.
        assert_eq!(
            texts,
            vec!["2025", "tion"],
            "font-aware sort must separate overlapping runs into distinct words",
        );
        // Each emitted word must also be X-localized to its own run, so
        // they don't visually collide downstream.
        assert_eq!(words.len(), 2);
    }

    /// Regression test for the ansys_13_command_reference.pdf
    /// "missing-space-after-period" bug.
    ///
    /// Reproduces the exact glyph configuration on page 2 line 13:
    /// `provisions.<space-in-different-font>The`. The `.` and `T` are
    /// both in `Myriad-Roman`; the intervening space glyph is in `R8604`
    /// — a tiny dedicated justification-pad font commonly emitted by
    /// professional typesetting engines. A naive (Y, raw_font, X) sort
    /// drops the orphan space (the bucket trims to empty and the word
    /// is never pushed) and leaves `.T` adjacent in the Myriad bucket,
    /// fusing them into a single token without a separator.
    ///
    /// With the effective-font preprocessing (a space's sort-font is the
    /// nearest preceding non-space char's font on the same baseline),
    /// the space re-homes into Myriad's bucket as an internal whitespace
    /// char, and the tokenizer yields `provisions. The` correctly.
    #[test]
    fn padding_space_in_dedicated_font_does_not_fuse_period_and_next_word() {
        let body = "TOUDBK+Myriad-Roman";
        let pad = "R8604"; // PDF justification-pad font.
        let y = 601.68;
        let h = 9.72;
        // `s . <space-in-pad-font> T h e` — geometry mirrors the dump
        // from page 2 of ansys_13_command_reference.pdf.
        let chars = vec![
            mk_char('s', 319.50, y, 3.85, h, body),
            mk_char('.', 323.35, y, 2.01, h, body),
            mk_char(' ', 325.42, y, 1.88, h, pad), // orphan in own font
            mk_char('T', 327.25, y, 4.83, h, body),
            mk_char('h', 332.07, y, 5.40, h, body),
            mk_char('e', 337.53, y, 4.87, h, body),
        ];

        let words = group_chars_into_words(&chars);
        // Concatenate the produced words with single spaces — the test
        // asserts the SEPARATION exists, not its rendered width.
        let joined = words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join("|");
        assert!(
            joined.contains(". The") || joined.contains(".|The") || joined.contains(". T"),
            "expected `. The` (space preserved between sentence boundary), \
             got `{joined}` — orphan space in pad-font bucket was dropped",
        );
        assert!(
            !joined.contains(".T"),
            "fused `.T` indicates the pad-font space was dropped and the \
             period got concatenated to the next word: `{joined}`",
        );
    }

    /// Sanity: a single monotonic run of chars in one font collapses to
    /// one word. Guards against the font-aware sort accidentally
    /// splitting normal text.
    #[test]
    fn single_font_run_stays_one_word() {
        let y = 100.0;
        let h = 8.0;
        let chars: Vec<TextChar> = "hello"
            .chars()
            .enumerate()
            .map(|(i, c)| mk_char(c, 10.0 + i as f32 * 5.0, y, 5.0, h, "Helvetica"))
            .collect();

        let words = group_chars_into_words(&chars);
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "hello");
    }
}
