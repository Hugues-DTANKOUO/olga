use rstar::{AABB, RTree, RTreeObject};

use super::{LAParams, TextBox, TextLine};

/// Greedy O(n²) grouping for small input sizes.
pub(super) fn group_lines_greedy(lines: &[TextLine], params: &LAParams) -> Vec<TextBox> {
    // Compute the median line height for gap threshold calculation.
    let median_line_height = {
        let mut heights: Vec<f32> = lines.iter().map(|l| l.bbox.height).collect();
        heights.sort_by(|a, b| a.total_cmp(b));
        heights[heights.len() / 2]
    };

    // Gap threshold: lines separated by more than this are in different boxes.
    // Add a small epsilon tolerance to avoid floating-point boundary issues
    // (e.g., 0.13 + 0.02 computing to 0.14999999 instead of 0.15 in f32).
    let gap_threshold = params.line_overlap_threshold * median_line_height + 1e-5;

    // Phase 1: Cluster lines by horizontal overlap.
    // Each cluster collects lines that share significant X extent.
    let mut clusters: Vec<Vec<TextLine>> = Vec::new();

    for line in lines {
        let mut merged = false;
        for cluster in &mut clusters {
            // Check if this line overlaps horizontally with any line
            // in the cluster (use the cluster's X extent).
            let cluster_x_min = cluster.iter().map(|l| l.bbox.x).fold(f32::MAX, f32::min);
            let cluster_x_max = cluster
                .iter()
                .map(|l| l.bbox.x + l.bbox.width)
                .fold(f32::MIN, f32::max);

            let overlap = horizontal_overlap_range(
                cluster_x_min,
                cluster_x_max,
                line.bbox.x,
                line.bbox.x + line.bbox.width,
            );

            if overlap > 0.3 {
                cluster.push(line.clone());
                merged = true;
                break;
            }
        }
        if !merged {
            clusters.push(vec![line.clone()]);
        }
    }

    // Phase 2: Within each cluster, merge consecutive lines by Y gap.
    let mut boxes: Vec<TextBox> = Vec::new();

    for cluster in &mut clusters {
        // Sort lines within cluster by Y (top to bottom).
        cluster.sort_by(|a, b| a.bbox.y.total_cmp(&b.bbox.y));

        let mut current_box_lines: Vec<TextLine> = vec![cluster[0].clone()];

        for line in cluster.iter().skip(1) {
            // Safe: current_box_lines is initialized with one element and only
            // grows (push) or resets to one element — it is never empty here.
            let Some(prev) = current_box_lines.last() else {
                continue;
            };
            let prev_bottom = prev.bbox.y + prev.bbox.height;
            let gap = line.bbox.y - prev_bottom;

            if gap <= gap_threshold {
                current_box_lines.push(line.clone());
            } else {
                boxes.push(TextBox::from_lines(current_box_lines));
                current_box_lines = vec![line.clone()];
            }
        }

        if !current_box_lines.is_empty() {
            boxes.push(TextBox::from_lines(current_box_lines));
        }
    }

    boxes
}

/// R-tree-backed grouping for larger input sizes.
///
/// Uses an R-tree spatial index to efficiently find nearby lines within the
/// gap threshold, then union-find to merge connected components.
pub(super) fn group_lines_into_boxes_rtree(lines: &[TextLine], params: &LAParams) -> Vec<TextBox> {
    // Compute the median line height for gap threshold calculation.
    let median_line_height = {
        let mut heights: Vec<f32> = lines.iter().map(|l| l.bbox.height).collect();
        heights.sort_by(|a, b| a.total_cmp(b));
        heights[heights.len() / 2]
    };

    let gap_threshold = params.line_overlap_threshold * median_line_height + 1e-5;

    // Phase 1: Build R-tree with line envelopes.
    let envelopes: Vec<LineEnvelope> = lines
        .iter()
        .enumerate()
        .map(|(idx, line)| {
            let x = line.bbox.x;
            let y = line.bbox.y;
            let x_max = line.bbox.x + line.bbox.width;
            let y_max = line.bbox.y + line.bbox.height;

            // Expand Y range by gap_threshold so R-tree queries catch neighbors.
            let aabb = AABB::from_corners([x, y - gap_threshold], [x_max, y_max + gap_threshold]);

            LineEnvelope { index: idx, aabb }
        })
        .collect();

    let tree = RTree::bulk_load(envelopes);

    // Phase 2: For each line, find neighbors and build union-find graph.
    let mut uf = UnionFind::new(lines.len());

    for (i, line) in lines.iter().enumerate() {
        let x = line.bbox.x;
        let y = line.bbox.y;
        let x_max = line.bbox.x + line.bbox.width;
        let y_max = line.bbox.y + line.bbox.height;

        let query_envelope =
            AABB::from_corners([x, y - gap_threshold], [x_max, y_max + gap_threshold]);

        for neighbor in tree.locate_in_envelope_intersecting(&query_envelope) {
            let j = neighbor.index;
            if i == j {
                continue;
            }

            let other = &lines[j];
            let gap = if line.bbox.y > other.bbox.y {
                // line is below other
                line.bbox.y - (other.bbox.y + other.bbox.height)
            } else {
                // line is above other
                other.bbox.y - (line.bbox.y + line.bbox.height)
            };

            // Check: Y-distance within gap_threshold and horizontal overlap > 0.3.
            if gap <= gap_threshold {
                let overlap = horizontal_overlap_range(
                    x,
                    x_max,
                    other.bbox.x,
                    other.bbox.x + other.bbox.width,
                );
                if overlap > 0.3 {
                    uf.union(i, j);
                }
            }
        }
    }

    // Phase 3: Group lines by connected component, build TextBoxes.
    let mut components: std::collections::HashMap<usize, Vec<TextLine>> =
        std::collections::HashMap::new();

    for (i, line) in lines.iter().enumerate() {
        let root = uf.find(i);
        components.entry(root).or_default().push(line.clone());
    }

    let mut boxes: Vec<TextBox> = Vec::new();

    for mut component in components.into_values() {
        // Sort lines within component by Y (top to bottom).
        component.sort_by(|a, b| a.bbox.y.total_cmp(&b.bbox.y));
        boxes.push(TextBox::from_lines(component));
    }

    boxes
}

/// Wrapper struct for R-tree indexing of text lines.
struct LineEnvelope {
    index: usize,
    aabb: AABB<[f32; 2]>,
}

impl RTreeObject for LineEnvelope {
    type Envelope = AABB<[f32; 2]>;

    fn envelope(&self) -> Self::Envelope {
        self.aabb
    }
}

/// Simple union-find (disjoint-set) structure with path compression and union by rank.
struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
}

impl UnionFind {
    fn new(size: usize) -> Self {
        Self {
            parent: (0..size).collect(),
            rank: vec![0; size],
        }
    }

    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            self.parent[x] = self.find(self.parent[x]);
        }
        self.parent[x]
    }

    fn union(&mut self, x: usize, y: usize) {
        let root_x = self.find(x);
        let root_y = self.find(y);

        if root_x == root_y {
            return;
        }

        if self.rank[root_x] < self.rank[root_y] {
            self.parent[root_x] = root_y;
        } else if self.rank[root_x] > self.rank[root_y] {
            self.parent[root_y] = root_x;
        } else {
            self.parent[root_y] = root_x;
            self.rank[root_x] += 1;
        }
    }
}

/// Compute the horizontal overlap ratio between two X-ranges.
///
/// Returns a value in [0.0, 1.0] indicating how much the horizontal
/// extents overlap, relative to the smaller width.
fn horizontal_overlap_range(a_left: f32, a_right: f32, b_left: f32, b_right: f32) -> f32 {
    let overlap_left = a_left.max(b_left);
    let overlap_right = a_right.min(b_right);
    let overlap = (overlap_right - overlap_left).max(0.0);

    let a_width = a_right - a_left;
    let b_width = b_right - b_left;
    let min_width = a_width.min(b_width);
    if min_width <= 0.0 {
        return 0.0;
    }

    overlap / min_width
}
