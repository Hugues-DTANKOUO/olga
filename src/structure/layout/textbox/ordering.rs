use super::{LAParams, TextBox};

/// Column-aware reading order with weighted distance scoring.
///
/// Uses a two-level approach:
/// 1. Detect column boundaries from horizontal gaps.
/// 2. Within each column, order boxes using a hybrid distance metric:
///    `flow × vertical_distance + (1 - flow) × area_distance` (informed
///    by pdfminer.six; see ATTRIBUTION.md).
///
/// When `flow = 1.0`, ordering is purely top-to-bottom.
/// When `flow = 0.0`, ordering is purely by spatial closeness.
/// When `flow = 0.5` (default), it's a balanced hybrid that follows
/// reading flow while keeping spatially related boxes together.
pub(super) fn order_by_columns(boxes: Vec<TextBox>, params: &LAParams) -> Vec<TextBox> {
    if boxes.len() <= 1 {
        return boxes;
    }

    let flow = params.boxes_flow.unwrap_or(0.5).clamp(0.0, 1.0);

    // Detect column boundaries.
    let column_boundaries = detect_column_boundaries(&boxes, params.column_gap_fraction);

    // Assign each box to a column.
    let mut columns: Vec<Vec<usize>> = vec![Vec::new(); column_boundaries.len() + 1];
    for (i, b) in boxes.iter().enumerate() {
        let col = column_boundaries
            .iter()
            .position(|&boundary| b.center_x() < boundary)
            .unwrap_or(column_boundaries.len());
        columns[col].push(i);
    }

    // Order boxes within each column using the weighted distance metric,
    // then concatenate columns left-to-right.
    let mut ordered = Vec::with_capacity(boxes.len());

    for col_indices in &columns {
        if col_indices.is_empty() {
            continue;
        }
        let col_order = order_by_weighted_distance(col_indices, &boxes, flow);
        for idx in col_order {
            ordered.push(boxes[idx].clone());
        }
    }

    // Safety: add any boxes not assigned to a column (shouldn't happen).
    let in_ordered: std::collections::HashSet<usize> =
        columns.iter().flat_map(|c| c.iter().copied()).collect();
    for (i, b) in boxes.iter().enumerate() {
        if !in_ordered.contains(&i) {
            ordered.push(b.clone());
        }
    }

    ordered
}

/// Order a subset of boxes using a weighted distance metric.
///
/// Starting from the topmost box, greedily select the nearest unvisited box
/// according to: `flow × vertical_dist + (1 - flow) × area_dist`. The metric
/// is informed by pdfminer.six (see ATTRIBUTION.md).
///
/// - `vertical_dist`: absolute Y difference between box centers (favors
///   top-to-bottom reading flow).
/// - `area_dist`: area of the union bounding box minus the individual areas
///   (favors spatially close boxes, regardless of direction).
pub(super) fn order_by_weighted_distance(
    indices: &[usize],
    boxes: &[TextBox],
    flow: f32,
) -> Vec<usize> {
    if indices.len() <= 1 {
        return indices.to_vec();
    }

    // Fast path: when flow is exactly 1.0, just sort by Y then X.
    if (flow - 1.0).abs() < f32::EPSILON {
        let mut sorted = indices.to_vec();
        sorted.sort_by(|&a, &b| {
            boxes[a]
                .bbox
                .y
                .total_cmp(&boxes[b].bbox.y)
                .then(boxes[a].bbox.x.total_cmp(&boxes[b].bbox.x))
        });
        return sorted;
    }

    let mut remaining: Vec<usize> = indices.to_vec();
    let mut result = Vec::with_capacity(indices.len());

    // Start with the topmost (then leftmost) box.
    remaining.sort_by(|&a, &b| {
        boxes[a]
            .bbox
            .y
            .total_cmp(&boxes[b].bbox.y)
            .then(boxes[a].bbox.x.total_cmp(&boxes[b].bbox.x))
    });

    let first = remaining.remove(0);
    result.push(first);

    while !remaining.is_empty() {
        // Safe: result always has at least one element (we pushed `first` above
        // and only append further elements in this loop).
        let Some(&current) = result.last() else {
            break;
        };
        let current_box = &boxes[current];

        // Find the nearest remaining box by weighted distance.
        // Safe: the while guard ensures remaining is non-empty.
        let Some((best_pos, _)) = remaining.iter().enumerate().min_by(|&(_, &a), &(_, &b)| {
            let da = weighted_distance(current_box, &boxes[a], flow);
            let db = weighted_distance(current_box, &boxes[b], flow);
            da.total_cmp(&db)
        }) else {
            break;
        };

        result.push(remaining.remove(best_pos));
    }

    result
}

/// Compute a weighted distance between two text boxes.
///
/// `distance = flow × vertical_dist + (1 - flow) × area_dist`
///
/// - `vertical_dist`: |center_y(a) - center_y(b)|, normalized by page height (~1.0).
/// - `area_dist`: area(union_bbox) - area(a) - area(b). This is the "wasted space"
///   in the union rectangle — small when boxes are close and similarly sized.
fn weighted_distance(a: &TextBox, b: &TextBox, flow: f32) -> f32 {
    let vertical_dist = (a.center_y() - b.center_y()).abs();

    let union_x = a.bbox.x.min(b.bbox.x);
    let union_y = a.bbox.y.min(b.bbox.y);
    let union_right = (a.bbox.x + a.bbox.width).max(b.bbox.x + b.bbox.width);
    let union_bottom = (a.bbox.y + a.bbox.height).max(b.bbox.y + b.bbox.height);
    let union_area = (union_right - union_x) * (union_bottom - union_y);
    let area_a = a.bbox.width * a.bbox.height;
    let area_b = b.bbox.width * b.bbox.height;
    let area_dist = (union_area - area_a - area_b).max(0.0);

    flow * vertical_dist + (1.0 - flow) * area_dist
}

/// Detect column boundaries as X-coordinates where no text box crosses.
///
/// A column boundary exists at position X if:
/// - The horizontal gap at X is wider than `gap_fraction` of the page width.
/// - Multiple text boxes exist on both sides of the gap.
fn detect_column_boundaries(boxes: &[TextBox], gap_fraction: f32) -> Vec<f32> {
    if boxes.len() < 2 {
        return Vec::new();
    }

    // Approximate page width as the span from leftmost to rightmost box edge.
    let page_left = boxes.iter().map(|b| b.bbox.x).fold(f32::MAX, f32::min);
    let page_right = boxes
        .iter()
        .map(|b| b.bbox.x + b.bbox.width)
        .fold(f32::MIN, f32::max);
    let page_width = page_right - page_left;
    if page_width <= 0.0 {
        return Vec::new();
    }

    let min_gap = gap_fraction * page_width;

    // Collect all box intervals on the X axis.
    let mut intervals: Vec<(f32, f32)> = boxes
        .iter()
        .map(|b| (b.bbox.x, b.bbox.x + b.bbox.width))
        .collect();
    intervals.sort_by(|a, b| a.0.total_cmp(&b.0));

    // Merge overlapping intervals to find gaps.
    let mut merged: Vec<(f32, f32)> = Vec::new();
    for interval in &intervals {
        if let Some(last) = merged.last_mut() {
            if interval.0 <= last.1 + min_gap * 0.1 {
                // Overlapping or very close — merge.
                last.1 = last.1.max(interval.1);
            } else {
                merged.push(*interval);
            }
        } else {
            merged.push(*interval);
        }
    }

    // Gaps between merged intervals are potential column boundaries.
    let mut boundaries = Vec::new();
    for window in merged.windows(2) {
        let gap = window[1].0 - window[0].1;
        if gap >= min_gap {
            // Column boundary at the midpoint of the gap.
            boundaries.push((window[0].1 + window[1].0) / 2.0);
        }
    }

    boundaries
}
