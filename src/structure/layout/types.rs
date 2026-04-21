//! Types for the layout analysis pipeline.
//!
//! These intermediate types flow through the three stages of layout analysis.
//! They are internal to the layout module and not part of the public API.

use crate::model::{BoundingBox, Primitive, PrimitiveKind};

// ---------------------------------------------------------------------------
// TextSpan — a single text primitive's geometry
// ---------------------------------------------------------------------------

/// A text span extracted from a Primitive, carrying the geometry and
/// typography information needed for layout analysis.
///
/// This is the input to Stage 1 (character → line grouping).
#[derive(Debug, Clone)]
pub struct TextSpan {
    /// Index of the source primitive in the page's primitive slice.
    pub primitive_index: usize,
    /// Bounding box (normalized 0.0–1.0 coordinates).
    pub bbox: BoundingBox,
    /// Font size in normalized units (relative to page height).
    /// Approximated from the bounding box height when not directly available.
    pub font_size: f32,
    /// Whether the text is bold.
    pub is_bold: bool,
    /// Whether the text is italic.
    pub is_italic: bool,
    /// The text content.
    pub text: String,
}

/// Extract text spans from a page of primitives.
///
/// Only `PrimitiveKind::Text` primitives are extracted. Non-text primitives
/// (lines, rectangles, images) are skipped — they may be used later by
/// table/list detectors but are not part of the text layout pipeline.
pub fn extract_text_spans(primitives: &[Primitive]) -> Vec<TextSpan> {
    let mut spans = Vec::new();
    for (i, prim) in primitives.iter().enumerate() {
        if let PrimitiveKind::Text {
            ref content,
            font_size,
            is_bold,
            is_italic,
            ..
        } = prim.kind
        {
            // Skip empty or whitespace-only text.
            if content.trim().is_empty() {
                continue;
            }
            spans.push(TextSpan {
                primitive_index: i,
                bbox: prim.bbox,
                font_size: if font_size > 0.0 {
                    font_size
                } else {
                    prim.bbox.height
                },
                is_bold,
                is_italic,
                text: content.clone(),
            });
        }
    }
    spans
}

// ---------------------------------------------------------------------------
// TextLine — a horizontal line of text spans
// ---------------------------------------------------------------------------

/// A line of text: a group of text spans on the same horizontal baseline.
///
/// Produced by Stage 1, consumed by Stage 2.
#[derive(Debug, Clone)]
pub struct TextLine {
    /// The text spans in this line, sorted left-to-right (ascending X).
    pub spans: Vec<TextSpan>,
    /// Bounding box enclosing all spans in this line.
    pub bbox: BoundingBox,
    /// Average font size of spans in this line.
    pub avg_font_size: f32,
}

impl TextLine {
    /// Compute the bounding box and average font size from the spans.
    pub fn from_spans(mut spans: Vec<TextSpan>) -> Self {
        assert!(!spans.is_empty(), "TextLine must have at least one span");

        // Sort spans by X coordinate (left to right).
        spans.sort_by(|a, b| a.bbox.x.total_cmp(&b.bbox.x));

        let bbox = enclosing_bbox(spans.iter().map(|s| &s.bbox));
        let avg_font_size = spans.iter().map(|s| s.font_size).sum::<f32>() / spans.len() as f32;

        Self {
            spans,
            bbox,
            avg_font_size,
        }
    }

    /// Full concatenated text of this line.
    pub fn text(&self) -> String {
        self.spans
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// All primitive indices covered by this line.
    pub fn primitive_indices(&self) -> Vec<usize> {
        self.spans.iter().map(|s| s.primitive_index).collect()
    }
}

// ---------------------------------------------------------------------------
// TextBox — a group of lines forming a logical text block
// ---------------------------------------------------------------------------

/// A text box: a group of vertically adjacent lines forming a logical
/// text block (e.g., a paragraph, a heading, a list item).
///
/// Produced by Stage 2, consumed by Stage 3 and the detector pipeline.
#[derive(Debug, Clone)]
pub struct TextBox {
    /// Lines in this box, sorted top-to-bottom (ascending Y).
    pub lines: Vec<TextLine>,
    /// Bounding box enclosing all lines in this box.
    pub bbox: BoundingBox,
    /// Average font size across all lines.
    pub avg_font_size: f32,
    /// Whether the majority of text in this box is bold.
    pub is_predominantly_bold: bool,
}

impl TextBox {
    /// Build a TextBox from a set of lines.
    pub fn from_lines(mut lines: Vec<TextLine>) -> Self {
        assert!(!lines.is_empty(), "TextBox must have at least one line");

        // Sort lines by Y coordinate (top to bottom).
        lines.sort_by(|a, b| a.bbox.y.total_cmp(&b.bbox.y));

        let bbox = enclosing_bbox(lines.iter().map(|l| &l.bbox));
        let total_spans: usize = lines.iter().map(|l| l.spans.len()).sum();
        let avg_font_size = if total_spans > 0 {
            lines
                .iter()
                .flat_map(|l| l.spans.iter())
                .map(|s| s.font_size)
                .sum::<f32>()
                / total_spans as f32
        } else {
            0.0
        };
        let bold_count = lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .filter(|s| s.is_bold)
            .count();
        let is_predominantly_bold = total_spans > 0 && bold_count * 2 > total_spans;

        Self {
            lines,
            bbox,
            avg_font_size,
            is_predominantly_bold,
        }
    }

    /// Full concatenated text of this box.
    pub fn text(&self) -> String {
        self.lines
            .iter()
            .map(|l| l.text())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// All primitive indices covered by this text box.
    pub fn primitive_indices(&self) -> Vec<usize> {
        self.lines
            .iter()
            .flat_map(|l| l.primitive_indices())
            .collect()
    }

    /// Vertical center of this text box.
    pub fn center_y(&self) -> f32 {
        self.bbox.y + self.bbox.height / 2.0
    }

    /// Horizontal center of this text box.
    pub fn center_x(&self) -> f32 {
        self.bbox.x + self.bbox.width / 2.0
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compute the enclosing bounding box of an iterator of bounding boxes.
pub(crate) fn enclosing_bbox<'a>(bboxes: impl Iterator<Item = &'a BoundingBox>) -> BoundingBox {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    for b in bboxes {
        min_x = min_x.min(b.x);
        min_y = min_y.min(b.y);
        max_x = max_x.max(b.x + b.width);
        max_y = max_y.max(b.y + b.height);
    }

    BoundingBox {
        x: min_x,
        y: min_y,
        width: (max_x - min_x).max(0.0),
        height: (max_y - min_y).max(0.0),
    }
}
