use crate::model::{BoundingBox, Primitive};

/// A span of text within a visual line, carrying its column position.
#[derive(Debug, Clone)]
pub struct AlignedSpan {
    /// Starting character column (0-based) for proportional layout.
    pub col_start: usize,
    /// The text content.
    pub text: String,
    /// Index of the source primitive in the page slice.
    pub prim_index: usize,
    /// Original bounding box (normalized 0–1).
    pub bbox: BoundingBox,
}

/// A visual line: a group of text spans that share the same vertical position.
#[derive(Debug, Clone)]
pub struct VisualLine {
    /// Spans sorted left-to-right, each with a character-column position.
    pub spans: Vec<AlignedSpan>,
    /// Merged bounding box covering all spans.
    pub bbox: BoundingBox,
    /// Indices of the source primitives in the page slice.
    pub prim_indices: Vec<usize>,
}

/// Page-level metrics for proportional layout.
#[derive(Debug, Clone)]
pub struct PageMetrics {
    /// Average character width in normalized page units.
    pub avg_char_width: f32,
    /// Total line width in character columns.
    pub line_width_chars: usize,
}

impl PageMetrics {
    /// Compute page metrics from a set of text primitives.
    ///
    /// Estimates the average character width by dividing each primitive's
    /// physical width by its character count, then taking the median.
    /// Falls back to 80 columns if no text is available.
    pub fn from_primitives(primitives: &[Primitive]) -> Self {
        let mut char_widths: Vec<f32> = Vec::new();

        for prim in primitives {
            if let Some(text) = prim.text_content() {
                let char_count = text.chars().count();
                if char_count > 0 && prim.bbox.width > 0.0 {
                    char_widths.push(prim.bbox.width / char_count as f32);
                }
            }
        }

        if char_widths.is_empty() {
            return Self {
                avg_char_width: 1.0 / 80.0,
                line_width_chars: 80,
            };
        }

        char_widths.sort_by(|a, b| a.total_cmp(b));
        let median = char_widths[char_widths.len() / 2];
        let line_width = (1.0 / median).round().clamp(40.0, 200.0) as usize;

        Self {
            avg_char_width: median,
            line_width_chars: line_width,
        }
    }
}
