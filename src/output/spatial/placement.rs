use super::words::RawWord;

#[derive(Clone)]
pub(super) struct PlacedText {
    pub(super) text: String,
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) w: f32,
    /// PDF glyph height carried through into normalized column-units. The
    /// downstream cross-font merger uses `h` directly (mirroring the
    /// within-bucket tokenizer's `gap > h * 0.3` break threshold) instead
    /// of recomputing a fragile median-width proxy from the placement
    /// vector — that proxy was correct on most documents but introduced
    /// regressions on pristine layouts (contract / earnings / monograph)
    /// where the whitespace distribution skewed the median.
    pub(super) h: f32,
    pub(super) link: Option<String>,
    /// `true` when the tokenizer trimmed a LEADING space glyph from the
    /// bucket that produced this word. Threaded through from
    /// `RawWord::absorbed_leading`. The cross-font merger reads this on
    /// the RIGHT fragment of a candidate merge — only the leading edge
    /// carries sentence-flow signal relative to the previous fragment.
    pub(super) absorbed_leading: bool,
    /// `true` when the tokenizer trimmed a TRAILING space glyph from the
    /// bucket that produced this word. Threaded through from
    /// `RawWord::absorbed_trailing`. The cross-font merger reads this on
    /// the LEFT fragment of a candidate merge — only the trailing edge
    /// carries sentence-flow signal relative to the next fragment.
    ///
    /// Splitting the original `absorbed_space` flag per-edge is load-
    /// bearing: a word can have a leading space from one source and no
    /// trailing space at all (e.g. the F55 space glyph preceding bold
    /// `French` on a CV layout, whose trailing edge is pure kerning
    /// against the `:` cell separator). The collapsed single-bool form
    /// conflated these directions and forced a sentence-flow pad of 1
    /// across structural table boundaries, emitting `French : Native`
    /// instead of `French  : Native`.
    pub(super) absorbed_trailing: bool,
}

pub(super) fn build_placed_texts(
    words: &[RawWord],
    min_x: f32,
    min_y: f32,
    cw: f32,
    ch: f32,
) -> Vec<PlacedText> {
    words
        .iter()
        .filter(|w| !w.text.is_empty())
        .map(|w| {
            let nx = (w.x - min_x) / cw;
            let ny = 1.0 - (w.y - min_y) / ch;
            let nw = w.w / cw;
            // Normalize `h` to the SAME unit as `x`/`w` (column units, not
            // row units). The merger compares forward gaps in `x` against
            // a multiple of `h`, so they must share a denominator. Picking
            // `cw` keeps the threshold semantically equal to "0.3 of a
            // glyph-width" — the same scale the tokenizer uses.
            let nh = w.h / cw;
            PlacedText {
                text: w.text.clone(),
                x: nx,
                y: ny,
                w: nw,
                h: nh,
                link: w.link.clone(),
                absorbed_leading: w.absorbed_leading,
                absorbed_trailing: w.absorbed_trailing,
            }
        })
        .collect()
}

pub(super) fn compute_target_cols(placed: &[PlacedText]) -> usize {
    let mut ratios: Vec<f32> = Vec::new();
    for p in placed {
        let chars = p.text.chars().count();
        if chars >= 2 && p.w > 0.01 {
            ratios.push(chars as f32 / p.w);
        }
    }
    if ratios.is_empty() {
        return 100;
    }
    ratios.sort_by(|a, b| a.total_cmp(b));
    let p90_idx = (ratios.len() as f32 * 0.90).ceil() as usize;
    let p90 = ratios[p90_idx.min(ratios.len() - 1)];
    (p90.round() as usize).clamp(60, 250)
}
