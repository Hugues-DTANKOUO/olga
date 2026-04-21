/// Detects headings based on font-size outliers relative to the document's
/// font histogram.
///
/// Confidence:
/// - 0.8 for bold text with large font size (wins over ParagraphDetector's 0.7)
/// - 0.5 for non-bold text with large font size
pub struct HeadingDetector {
    /// Percentile threshold: font sizes in the top `percentile` fraction
    /// are heading candidates. Default: 0.10 (top 10%).
    pub percentile: f32,
    /// Minimum ratio of primitive font size to body font size to qualify
    /// as a heading. Default: 1.15 (15% larger than body text).
    pub min_size_ratio: f32,
    /// Confidence for bold heading candidates. Default: 0.8.
    ///
    /// Must be strictly greater than ParagraphDetector's 0.7 so that bold
    /// headings win the conflict-resolution tiebreak in `apply_detectors`.
    pub bold_confidence: f32,
    /// Confidence for non-bold heading candidates. Default: 0.5.
    pub plain_confidence: f32,
    /// Minimum text length (in chars) for a primitive to be considered
    /// a heading. Very short strings (1-2 chars) are usually page numbers
    /// or bullets, not headings. Default: 3.
    pub min_text_len: usize,
    /// Maximum text length (in chars). Very long text is unlikely to be
    /// a heading. Default: 200.
    pub max_text_len: usize,
}

impl HeadingDetector {
    /// Validate configuration values and clamp to sensible ranges.
    ///
    /// Returns a list of warnings for any clamped values. Call this to
    /// ensure the detector won't silently produce wrong output.
    pub fn validate(&mut self) -> Vec<String> {
        let mut issues = Vec::new();

        if self.percentile <= 0.0 || self.percentile > 1.0 {
            issues.push(format!(
                "HeadingDetector: percentile {} clamped to 0.10 (must be in (0.0, 1.0])",
                self.percentile
            ));
            self.percentile = 0.10;
        }
        if self.min_size_ratio < 1.0 {
            issues.push(format!(
                "HeadingDetector: min_size_ratio {} clamped to 1.0 (must be >= 1.0)",
                self.min_size_ratio
            ));
            self.min_size_ratio = 1.0;
        }
        if self.bold_confidence < 0.0 || self.bold_confidence > 1.0 {
            issues.push(format!(
                "HeadingDetector: bold_confidence {} clamped to 0.8 (must be in [0.0, 1.0])",
                self.bold_confidence
            ));
            self.bold_confidence = 0.8;
        }
        if self.plain_confidence < 0.0 || self.plain_confidence > 1.0 {
            issues.push(format!(
                "HeadingDetector: plain_confidence {} clamped to 0.5 (must be in [0.0, 1.0])",
                self.plain_confidence
            ));
            self.plain_confidence = 0.5;
        }

        issues
    }
}

impl Default for HeadingDetector {
    fn default() -> Self {
        Self {
            percentile: 0.10,
            min_size_ratio: 1.15,
            bold_confidence: 0.8,
            plain_confidence: 0.5,
            min_text_len: 3,
            max_text_len: 200,
        }
    }
}
