use super::*;

impl TableDetector {
    /// Detect borderless tables by generating virtual edges from text alignment,
    /// then running the standard lattice pipeline (intersections → cells → tables).
    ///
    /// Virtual vertical edges are derived from three clustering passes on
    /// character x-positions (left, center, right) and virtual horizontal
    /// edges from a single pass on top positions. These edges feed into the
    /// same intersection → cell → table pipeline used by the lattice path.
    /// The three-pass vertical clustering scheme is informed by pdfplumber's
    /// `words_to_edges_v`; see ATTRIBUTION.md.
    pub(super) fn detect_stream(
        &self,
        page: &[Primitive],
        context: &DetectionContext,
    ) -> Vec<DetectedHint> {
        // Collect unhinted text primitives.
        let text_items: Vec<TextItem> = page
            .iter()
            .filter_map(|p| {
                if p.has_format_hints() {
                    return None;
                }
                if let PrimitiveKind::Text { ref content, .. } = p.kind {
                    if content.trim().is_empty() {
                        return None;
                    }
                    Some(TextItem {
                        x: p.bbox.x,
                        y: p.bbox.y,
                        width: p.bbox.width,
                    })
                } else {
                    None
                }
            })
            .collect();

        if text_items.len() < self.min_columns * self.min_rows {
            return Vec::new();
        }

        let x_tolerance = self.adaptive_x_tolerance(context);

        // --- Generate vertical edges from x-alignment clusters ---
        let x_clusters = cluster_x_positions(&text_items, x_tolerance);
        if x_clusters.len() < self.min_columns {
            return Vec::new();
        }

        // Stream mode guard 1: require minimum column separation.
        // If the widest gap between consecutive column centroids is less than
        // 5% of page width, the "columns" are likely just text alignment noise
        // (e.g., body text with consistent left margin). Real borderless tables
        // have visually distinct columns with meaningful separation.
        if x_clusters.len() >= 2 {
            let max_gap = x_clusters
                .windows(2)
                .map(|w| (w[1] - w[0]).abs())
                .fold(0.0_f32, f32::max);
            if max_gap < 0.05 {
                return Vec::new();
            }
        }

        // Note: per-column item count guard removed — the 3-pass clustering
        // (left/center/right edges) produces multiple clusters per visual column,
        // making it unreliable to count items per cluster. Instead, the fill-rate
        // post-filter (>= 30%) catches sparse false positives downstream.

        // --- Generate horizontal edges from y-alignment clusters ---
        let y_tolerance = x_tolerance * 2.0;
        let y_clusters = cluster_y_positions(&text_items, y_tolerance);
        if y_clusters.len() < self.min_rows {
            return Vec::new();
        }

        // Determine horizontal extent for edge boundaries.
        let x_min = text_items.iter().map(|i| i.x).fold(f32::INFINITY, f32::min);
        let x_max = text_items
            .iter()
            .map(|i| i.x + i.width)
            .fold(0.0_f32, f32::max);

        // Segment Y clusters into table regions using y_gap_factor.
        // Rows separated by more than y_gap_factor × median_gap belong to
        // different tables. Each region gets its own set of virtual edges,
        // preventing a giant empty cell from merging distant tables.
        let y_regions = self.segment_y_clusters(&y_clusters);

        let mut hints = Vec::new();

        for region_ys in &y_regions {
            if region_ys.len() < self.min_rows {
                continue;
            }

            let region_y_min = region_ys[0];
            let region_y_max = region_ys[region_ys.len() - 1];

            let mut virtual_edges = Vec::with_capacity(x_clusters.len() + 1 + region_ys.len() + 1);

            // Vertical edges scoped to this region's Y extent.
            for &x in &x_clusters {
                virtual_edges.push(edges::Edge {
                    orientation: edges::Orientation::Vertical,
                    position: x - x_tolerance * 0.5,
                    start: region_y_min - y_tolerance,
                    end: region_y_max + y_tolerance,
                });
            }
            virtual_edges.push(edges::Edge {
                orientation: edges::Orientation::Vertical,
                position: x_max + x_tolerance * 0.5,
                start: region_y_min - y_tolerance,
                end: region_y_max + y_tolerance,
            });

            // Horizontal edges for this region's rows.
            for &y in region_ys {
                virtual_edges.push(edges::Edge {
                    orientation: edges::Orientation::Horizontal,
                    position: y - y_tolerance * 0.5,
                    start: x_min - x_tolerance,
                    end: x_max + x_tolerance,
                });
            }
            virtual_edges.push(edges::Edge {
                orientation: edges::Orientation::Horizontal,
                position: region_y_max + y_tolerance * 0.5,
                start: x_min - x_tolerance,
                end: x_max + x_tolerance,
            });

            // Run the standard lattice pipeline on this region's edges.
            // Use Relaxed cell validation since the virtual grid is
            // approximate and strict 4-corner validation would reject
            // valid cells at grid boundaries.
            let region_hints = self.edges_to_hints(
                &virtual_edges,
                page,
                self.stream_confidence,
                edges::CellValidation::Relaxed,
            );

            // Post-filter: reject tables with low cell fill-rate.
            // A real borderless table should have content in most cells.
            // If fewer than 30% of the grid cells contain text, this is
            // likely a false positive from scattered text alignment.
            //
            // We count **distinct occupied cells** (unique row/col pairs)
            // rather than total hints, because a single cell can contain
            // many word-level primitives that would inflate the numerator.
            //
            // Because `cluster_x_positions` generates clusters from three
            // alignment passes (left/center/right edges), x_clusters.len()
            // can be ~3× the visual column count. We count distinct left-edge
            // positions (the true visual columns) for the denominator.
            if !region_hints.is_empty() {
                let visual_col_positions = {
                    let mut x0s: Vec<f32> = text_items.iter().map(|i| i.x).collect();
                    x0s.sort_by(|a, b| a.total_cmp(b));
                    x0s.dedup_by(|a, b| (*a - *b).abs() <= x_tolerance);
                    x0s
                };
                let visual_cols = visual_col_positions.len().max(1);

                // Count distinct occupied (row, col) cells by assigning
                // each text item in the region to its nearest row cluster
                // and visual column.
                let region_y_min_bound = region_ys[0] - y_tolerance;
                let region_y_max_bound = region_ys[region_ys.len() - 1] + y_tolerance;
                let mut occupied = std::collections::HashSet::new();
                for item in &text_items {
                    if item.y < region_y_min_bound || item.y > region_y_max_bound {
                        continue;
                    }
                    let row = region_ys
                        .iter()
                        .enumerate()
                        .min_by(|(_, a), (_, b)| {
                            (item.y - *a).abs().total_cmp(&(item.y - *b).abs())
                        })
                        .map(|(i, _)| i);
                    let col = visual_col_positions
                        .iter()
                        .enumerate()
                        .min_by(|(_, a), (_, b)| {
                            (item.x - *a).abs().total_cmp(&(item.x - *b).abs())
                        })
                        .map(|(i, _)| i);
                    if let (Some(r), Some(c)) = (row, col) {
                        occupied.insert((r, c));
                    }
                }

                let n_rows = region_ys.len();
                let grid_size = visual_cols * n_rows;
                let fill_rate = occupied.len() as f32 / grid_size as f32;
                if fill_rate >= 0.3 {
                    hints.extend(region_hints);
                }
            }
        }

        hints
    }

    /// Segment Y cluster centroids into separate table regions.
    ///
    /// Computes the median gap between consecutive Y centroids. Any gap
    /// exceeding `y_gap_factor × median_gap` triggers a split. Returns
    /// a list of centroid slices, each representing a contiguous region
    /// that becomes an independent virtual grid.
    fn segment_y_clusters(&self, y_clusters: &[f32]) -> Vec<Vec<f32>> {
        if y_clusters.len() < 2 {
            return vec![y_clusters.to_vec()];
        }

        let gaps: Vec<f32> = y_clusters.windows(2).map(|w| (w[1] - w[0]).abs()).collect();

        let median_gap = {
            let mut sorted = gaps.clone();
            sorted.sort_by(|a, b| a.total_cmp(b));
            let n = sorted.len();
            if n.is_multiple_of(2) {
                (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
            } else {
                sorted[n / 2]
            }
        };

        if median_gap < 1e-6 {
            return vec![y_clusters.to_vec()];
        }

        let threshold = median_gap * self.y_gap_factor;

        let mut regions: Vec<Vec<f32>> = Vec::new();
        let mut current = vec![y_clusters[0]];

        for (i, &gap) in gaps.iter().enumerate() {
            if gap > threshold {
                regions.push(current);
                current = vec![y_clusters[i + 1]];
            } else {
                current.push(y_clusters[i + 1]);
            }
        }
        regions.push(current);

        regions
    }

    /// Compute an adaptive X-tolerance when layout analysis results are
    /// available. Uses the median textbox width × 0.15, clamped to
    /// [0.005, 0.05]. Falls back to `self.x_tolerance` when no layout
    /// context is present.
    fn adaptive_x_tolerance(&self, context: &DetectionContext) -> f32 {
        if let Some(ref boxes) = context.text_boxes
            && !boxes.is_empty()
        {
            let mut widths: Vec<f32> = boxes.iter().map(|b| b.bbox.width).collect();
            widths.sort_by(|a, b| a.total_cmp(b));
            let n = widths.len();
            let median_width = if n.is_multiple_of(2) {
                (widths[n / 2 - 1] + widths[n / 2]) / 2.0
            } else {
                widths[n / 2]
            };
            return (median_width * 0.15).clamp(0.005, 0.05);
        }
        self.x_tolerance
    }
}

// ---------------------------------------------------------------------------
// Y-gap splitting for stream mode
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub(super) struct TextItem {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) width: f32,
}

// ---------------------------------------------------------------------------
// Clustering helpers
// ---------------------------------------------------------------------------

/// Cluster text items into vertical column positions.
///
/// Runs **three separate clustering passes** on left edge (x0), center
/// ((x0+x1)/2), and right edge (x1). Each pass independently finds groups
/// of items that share a common alignment on that axis. The surviving
/// cluster centroids from all three passes are then merged (deduplicating
/// positions within `tolerance` of each other) to produce the final column
/// positions.
///
/// A cluster must contain at least `min_word_count` items to survive,
/// which filters out noise from isolated items. The three-pass structure
/// is informed by pdfplumber's `words_to_edges_v`; see ATTRIBUTION.md.
pub(super) fn cluster_x_positions(items: &[TextItem], tolerance: f32) -> Vec<f32> {
    // With our min_rows × min_columns check upstream, 2 is a safe floor
    // that avoids false columns from a single stray item. (Comparable
    // techniques in pdfplumber use a threshold of 3 by default.)
    let min_word_count = 2;

    // Pass 1: cluster by left edge (x0).
    let mut x0s: Vec<f32> = items.iter().map(|i| i.x).collect();
    x0s.sort_by(|a, b| a.total_cmp(b));
    let clusters_x0 = cluster_sorted_positions_counted(&x0s, tolerance, min_word_count);

    // Pass 2: cluster by center ((x0 + x1) / 2).
    let mut centers: Vec<f32> = items.iter().map(|i| i.x + i.width / 2.0).collect();
    centers.sort_by(|a, b| a.total_cmp(b));
    let clusters_center = cluster_sorted_positions_counted(&centers, tolerance, min_word_count);

    // Pass 3: cluster by right edge (x1).
    let mut x1s: Vec<f32> = items.iter().map(|i| i.x + i.width).collect();
    x1s.sort_by(|a, b| a.total_cmp(b));
    let clusters_x1 = cluster_sorted_positions_counted(&x1s, tolerance, min_word_count);

    // Merge all surviving centroids from the 3 passes, then deduplicate
    // positions within `tolerance` of each other so overlapping alignments
    // (e.g., left of col B ≈ right of col A) don't produce duplicate columns.
    let mut all: Vec<f32> =
        Vec::with_capacity(clusters_x0.len() + clusters_center.len() + clusters_x1.len());
    all.extend_from_slice(&clusters_x0);
    all.extend_from_slice(&clusters_center);
    all.extend_from_slice(&clusters_x1);
    all.sort_by(|a, b| a.total_cmp(b));
    dedup_close(&mut all, tolerance);
    all
}

/// Cluster sorted positions, keeping only clusters with at least
/// `min_count` members.
fn cluster_sorted_positions_counted(
    positions: &[f32],
    tolerance: f32,
    min_count: usize,
) -> Vec<f32> {
    if positions.is_empty() {
        return Vec::new();
    }

    let mut clusters: Vec<Vec<f32>> = vec![vec![positions[0]]];

    for &pos in &positions[1..] {
        let last_idx = clusters.len() - 1;
        let last_cluster = &clusters[last_idx];
        let centroid = last_cluster.iter().sum::<f32>() / last_cluster.len() as f32;

        if (pos - centroid).abs() <= tolerance {
            clusters[last_idx].push(pos);
        } else {
            clusters.push(vec![pos]);
        }
    }

    clusters
        .iter()
        .filter(|c| c.len() >= min_count)
        .map(|c| c.iter().sum::<f32>() / c.len() as f32)
        .collect()
}

/// Remove positions that are within `tolerance` of their predecessor
/// (in-place on a sorted slice). Keeps the first representative of each
/// group.
fn dedup_close(sorted: &mut Vec<f32>, tolerance: f32) {
    if sorted.len() <= 1 {
        return;
    }
    let mut write = 1;
    for read in 1..sorted.len() {
        if (sorted[read] - sorted[write - 1]).abs() > tolerance {
            sorted[write] = sorted[read];
            write += 1;
        }
    }
    sorted.truncate(write);
}

fn cluster_y_positions(items: &[TextItem], tolerance: f32) -> Vec<f32> {
    let mut positions: Vec<f32> = items.iter().map(|i| i.y).collect();
    positions.sort_by(|a, b| a.total_cmp(b));
    cluster_sorted_positions(&positions, tolerance)
}

fn cluster_sorted_positions(positions: &[f32], tolerance: f32) -> Vec<f32> {
    // Delegate with min_count=1: every cluster survives.
    cluster_sorted_positions_counted(positions, tolerance, 1)
}
