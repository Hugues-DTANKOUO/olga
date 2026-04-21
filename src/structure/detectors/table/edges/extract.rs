//! Step 1 of the edges pipeline — turn raw `Line` and `Rectangle` primitives
//! into axis-aligned [`Edge`]s in normalized page coordinates.
//!
//! Nearly-horizontal / nearly-vertical lines (within `snap_tolerance` of
//! axis-aligned) are accepted; everything else is dropped. Rectangles produce
//! their 4 bounding edges, skipping the ones that would be shorter than
//! `min_edge_length`. Thick strokes are rejected as decorative borders.

use super::types::{Edge, EdgeConfig, Orientation};
use crate::model::{Primitive, PrimitiveKind};

/// Extract edges from a page of primitives (Step 1).
///
/// Processes `Line` and `Rectangle` primitives. Lines that are nearly
/// horizontal or nearly vertical (within `snap_tolerance` of axis-aligned)
/// become edges. Rectangles produce 4 axis-aligned edges.
///
/// Edges shorter than `config.min_edge_length` are discarded.
pub fn extract_edges(page: &[Primitive], config: &EdgeConfig) -> Vec<Edge> {
    let mut edges = Vec::new();

    for prim in page {
        match &prim.kind {
            PrimitiveKind::Line {
                start,
                end,
                thickness,
                ..
            } => {
                // Skip lines that are too thick (decorative).
                if *thickness > config.max_line_thickness {
                    continue;
                }

                let dx = (end.x - start.x).abs();
                let dy = (end.y - start.y).abs();

                if dy <= config.snap_tolerance && dx > config.min_edge_length {
                    // Horizontal line.
                    let y = (start.y + end.y) / 2.0;
                    let x_min = start.x.min(end.x);
                    let x_max = start.x.max(end.x);
                    edges.push(Edge {
                        orientation: Orientation::Horizontal,
                        position: y,
                        start: x_min,
                        end: x_max,
                    });
                } else if dx <= config.snap_tolerance && dy > config.min_edge_length {
                    // Vertical line.
                    let x = (start.x + end.x) / 2.0;
                    let y_min = start.y.min(end.y);
                    let y_max = start.y.max(end.y);
                    edges.push(Edge {
                        orientation: Orientation::Vertical,
                        position: x,
                        start: y_min,
                        end: y_max,
                    });
                }
            }
            PrimitiveKind::Rectangle {
                stroke_thickness, ..
            } => {
                // Extract 4 edges from the rectangle's bounding box.
                let b = &prim.bbox;

                // Skip tiny rectangles (likely dots or artifacts).
                if b.width < config.min_edge_length && b.height < config.min_edge_length {
                    continue;
                }

                // Skip rectangles with very thick strokes (decorative).
                if let Some(t) = stroke_thickness
                    && *t > config.max_line_thickness
                {
                    continue;
                }

                let x0 = b.x;
                let y0 = b.y;
                let x1 = b.right();
                let y1 = b.bottom();

                // Top edge (horizontal).
                if b.width >= config.min_edge_length {
                    edges.push(Edge {
                        orientation: Orientation::Horizontal,
                        position: y0,
                        start: x0,
                        end: x1,
                    });
                    // Bottom edge.
                    edges.push(Edge {
                        orientation: Orientation::Horizontal,
                        position: y1,
                        start: x0,
                        end: x1,
                    });
                }
                // Left edge (vertical).
                if b.height >= config.min_edge_length {
                    edges.push(Edge {
                        orientation: Orientation::Vertical,
                        position: x0,
                        start: y0,
                        end: y1,
                    });
                    // Right edge.
                    edges.push(Edge {
                        orientation: Orientation::Vertical,
                        position: x1,
                        start: y0,
                        end: y1,
                    });
                }
            }
            _ => {}
        }
    }

    edges
}
