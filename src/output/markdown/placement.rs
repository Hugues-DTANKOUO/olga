use super::words::RawWord;

#[derive(Clone)]
pub(super) struct PlacedWord {
    pub(super) text: String,
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) w: f32,
    /// PDF glyph height carried through into normalized column-units.
    /// Used by the cross-font merger to mirror the within-bucket
    /// tokenizer's `gap > h * 0.3` break threshold exactly — see the
    /// twin field on `spatial::placement::PlacedText`.
    pub(super) h: f32,
    pub(super) is_bold: bool,
    pub(super) is_italic: bool,
    pub(super) link: Option<String>,
    /// Threaded through from `RawWord::absorbed_leading`. Read by the
    /// cross-font merger on the RIGHT fragment of a merge candidate —
    /// see `spatial::placement::PlacedText` for the full directional
    /// rationale and the structural case that required the split.
    pub(super) absorbed_leading: bool,
    /// Threaded through from `RawWord::absorbed_trailing`. Read by the
    /// cross-font merger on the LEFT fragment of a merge candidate —
    /// see `spatial::placement::PlacedText` for the full directional
    /// rationale and the structural case that required the split.
    pub(super) absorbed_trailing: bool,
}

pub(super) fn build_placed_words(
    words: &[RawWord],
    min_x: f32,
    min_y: f32,
    cw: f32,
    ch: f32,
) -> Vec<PlacedWord> {
    words
        .iter()
        .filter(|w| !w.text.is_empty())
        .map(|w| {
            let nx = (w.x - min_x) / cw;
            let ny = 1.0 - (w.y - min_y) / ch;
            let nw = w.w / cw;
            // Normalize `h` in the SAME units as `x`/`w` (column units).
            // The merger compares forward gaps in x against a multiple
            // of `h`, so they must share a denominator — see
            // `spatial::placement::build_placed_texts`.
            let nh = w.h / cw;
            PlacedWord {
                text: w.text.clone(),
                x: nx,
                y: ny,
                w: nw,
                h: nh,
                is_bold: w.is_bold,
                is_italic: w.is_italic,
                link: w.link.clone(),
                absorbed_leading: w.absorbed_leading,
                absorbed_trailing: w.absorbed_trailing,
            }
        })
        .collect()
}

pub(super) fn compute_target_cols(placed: &[PlacedWord]) -> usize {
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
