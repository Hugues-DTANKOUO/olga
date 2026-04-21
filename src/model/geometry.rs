//! Geometric types for the Olga pipeline.
//!
//! All coordinates are normalized to 0.0–1.0 relative to the effective
//! page dimensions (after rotation, using CropBox). Y=0.0 is the TOP of the page.

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// Point
// ---------------------------------------------------------------------------

/// A 2D point in normalized coordinates (0.0–1.0).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Euclidean distance to another point (in normalized coordinate space).
    pub fn distance(&self, other: &Point) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}

// ---------------------------------------------------------------------------
// BoundingBox
// ---------------------------------------------------------------------------

/// Axis-aligned bounding box in normalized page coordinates.
///
/// - `x`, `y`: top-left corner (0.0–1.0)
/// - `width`, `height`: extent (0.0–1.0)
///
/// All values are relative to the effective page dimensions (after rotation).
/// Y=0.0 is the TOP of the page.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl BoundingBox {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Right edge (x + width).
    #[inline]
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    /// Bottom edge (y + height).
    #[inline]
    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    /// Center point.
    pub fn center(&self) -> Point {
        Point::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// Area of the bounding box.
    #[inline]
    pub fn area(&self) -> f32 {
        self.width * self.height
    }

    /// Returns `true` if this box overlaps with `other` (non-zero intersection).
    pub fn overlaps(&self, other: &BoundingBox) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }

    /// Returns the intersection of two bounding boxes, or `None` if they don't overlap.
    pub fn intersection(&self, other: &BoundingBox) -> Option<BoundingBox> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());

        if right > x && bottom > y {
            Some(BoundingBox::new(x, y, right - x, bottom - y))
        } else {
            None
        }
    }

    /// Returns `true` if this box fully contains `other`.
    pub fn contains(&self, other: &BoundingBox) -> bool {
        self.x <= other.x
            && self.y <= other.y
            && self.right() >= other.right()
            && self.bottom() >= other.bottom()
    }

    /// Returns `true` if this box contains the given point.
    pub fn contains_point(&self, point: &Point) -> bool {
        point.x >= self.x
            && point.x <= self.right()
            && point.y >= self.y
            && point.y <= self.bottom()
    }

    /// Returns the smallest bounding box that contains both `self` and `other`.
    pub fn merge(&self, other: &BoundingBox) -> BoundingBox {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        BoundingBox::new(x, y, right - x, bottom - y)
    }

    /// Minimum distance between the edges of two bounding boxes.
    /// Returns 0.0 if they overlap.
    pub fn distance(&self, other: &BoundingBox) -> f32 {
        let dx = if self.right() < other.x {
            other.x - self.right()
        } else if other.right() < self.x {
            self.x - other.right()
        } else {
            0.0
        };

        let dy = if self.bottom() < other.y {
            other.y - self.bottom()
        } else if other.bottom() < self.y {
            self.y - other.bottom()
        } else {
            0.0
        };

        (dx * dx + dy * dy).sqrt()
    }

    /// Vertical gap between this box and another (positive = other is below,
    /// negative = overlapping vertically).
    pub fn vertical_gap(&self, other: &BoundingBox) -> f32 {
        other.y - self.bottom()
    }

    /// Horizontal gap between this box and another (positive = other is to the right,
    /// negative = overlapping horizontally).
    pub fn horizontal_gap(&self, other: &BoundingBox) -> f32 {
        other.x - self.right()
    }

    /// Returns `true` if this box and `other` have significant horizontal overlap.
    /// `threshold` is the fraction of the smaller width that must overlap (0.0–1.0).
    pub fn horizontally_aligned(&self, other: &BoundingBox, threshold: f32) -> bool {
        let overlap_start = self.x.max(other.x);
        let overlap_end = self.right().min(other.right());
        let overlap = (overlap_end - overlap_start).max(0.0);
        let min_width = self.width.min(other.width);
        if min_width <= 0.0 {
            return false;
        }
        overlap / min_width >= threshold
    }

    /// Returns `true` if this box and `other` share similar X alignment
    /// (left edges within `tolerance`).
    pub fn left_aligned(&self, other: &BoundingBox, tolerance: f32) -> bool {
        (self.x - other.x).abs() <= tolerance
    }

    /// Validates that this bounding box has sensible normalized values.
    pub fn is_valid(&self) -> bool {
        self.x >= 0.0
            && self.y >= 0.0
            && self.width >= 0.0
            && self.height >= 0.0
            && self.right() <= 1.0 + f32::EPSILON
            && self.bottom() <= 1.0 + f32::EPSILON
    }
}

impl fmt::Display for BoundingBox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[({:.3}, {:.3}) {}x{}]",
            self.x, self.y, self.width, self.height
        )
    }
}

// ---------------------------------------------------------------------------
// RawBox (pre-normalization coordinates)
// ---------------------------------------------------------------------------

/// Raw rectangle in absolute (points) coordinates, as stored in the PDF/format.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RawBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl RawBox {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

// ---------------------------------------------------------------------------
// PageDimensions
// ---------------------------------------------------------------------------

/// Page dimensions and transformation metadata.
///
/// Used to normalize raw coordinates (from PDFs, etc.) into the 0.0–1.0 range.
/// The "effective" dimensions account for rotation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PageDimensions {
    /// Physical page rectangle (PDF MediaBox or equivalent).
    pub media_box: RawBox,
    /// Visible page rectangle (PDF CropBox, defaults to media_box).
    pub crop_box: RawBox,
    /// Page rotation in degrees (0, 90, 180, 270). Must be a multiple of 90.
    pub rotation: u16,
    /// Effective width after rotation is applied.
    pub effective_width: f32,
    /// Effective height after rotation is applied.
    pub effective_height: f32,
}

impl PageDimensions {
    /// Create PageDimensions from a media box, optional crop box, and rotation.
    ///
    /// If `crop_box` is `None`, it defaults to `media_box`.
    /// Rotation must be 0, 90, 180, or 270.
    pub fn new(media_box: RawBox, crop_box: Option<RawBox>, rotation: u16) -> Self {
        let rotation = match rotation {
            0 | 90 | 180 | 270 => rotation,
            _ => 0, // Invalid rotation — default to 0, log warning in caller
        };
        let effective_box = crop_box.unwrap_or(media_box);
        let (effective_width, effective_height) = match rotation {
            90 | 270 => (effective_box.height, effective_box.width),
            _ => (effective_box.width, effective_box.height),
        };
        Self {
            media_box,
            crop_box: crop_box.unwrap_or(media_box),
            rotation,
            effective_width,
            effective_height,
        }
    }

    /// Normalize a raw point (in format-native coordinates) to 0.0–1.0.
    ///
    /// For PDF: raw coordinates have Y=0 at BOTTOM. This method flips Y.
    /// The `flip_y` parameter controls whether Y-axis inversion is applied
    /// (true for PDF, false for formats where Y=0 is already at top).
    pub fn normalize_point(&self, raw_x: f32, raw_y: f32, flip_y: bool) -> Point {
        let (rx, ry) = self.apply_rotation(raw_x, raw_y, flip_y);
        Point::new(
            (rx / self.effective_width).clamp(0.0, 1.0),
            (ry / self.effective_height).clamp(0.0, 1.0),
        )
    }

    /// Normalize a raw bounding box to 0.0–1.0.
    ///
    /// `flip_y`: true for PDF (Y=0 at bottom), false otherwise.
    pub fn normalize_bbox(
        &self,
        raw_x: f32,
        raw_y: f32,
        raw_width: f32,
        raw_height: f32,
        flip_y: bool,
    ) -> BoundingBox {
        let (top_left_x, top_left_y) = if flip_y {
            self.apply_rotation(raw_x, raw_y + raw_height, flip_y)
        } else {
            self.apply_rotation(raw_x, raw_y, flip_y)
        };

        let (norm_w, norm_h) = match self.rotation {
            90 | 270 => (raw_height, raw_width),
            _ => (raw_width, raw_height),
        };

        BoundingBox::new(
            (top_left_x / self.effective_width).clamp(0.0, 1.0),
            (top_left_y / self.effective_height).clamp(0.0, 1.0),
            (norm_w / self.effective_width).clamp(0.0, 1.0),
            (norm_h / self.effective_height).clamp(0.0, 1.0),
        )
    }

    /// Apply rotation and optional Y-flip to raw coordinates.
    fn apply_rotation(&self, raw_x: f32, raw_y: f32, flip_y: bool) -> (f32, f32) {
        let cx = raw_x - self.crop_box.x;
        let cy = raw_y - self.crop_box.y;

        let cy = if flip_y {
            self.crop_box.height - cy
        } else {
            cy
        };

        match self.rotation {
            0 => (cx, cy),
            90 => (cy, self.crop_box.width - cx),
            180 => (self.crop_box.width - cx, self.crop_box.height - cy),
            270 => (self.crop_box.height - cy, cx),
            _ => (cx, cy),
        }
    }
}

impl fmt::Display for PageDimensions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Page({}x{}, rot={}°)",
            self.effective_width, self.effective_height, self.rotation
        )
    }
}
