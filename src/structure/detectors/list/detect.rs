use crate::model::{HintKind, Primitive, PrimitiveKind, SemanticHint};

use super::helpers::{dominant_box_left_margin, dominant_left_margin, has_text_gap};
use super::prefix::{detect_definition_prefix, detect_list_prefix};
use super::{DetectedHint, ListDetector};

impl ListDetector {
    /// Detect list items using pre-grouped text boxes from layout analysis.
    ///
    /// Each textbox's first line is checked for a bullet/number prefix.
    /// Indentation is measured at the textbox level (box.bbox.x), which is
    /// more robust than per-primitive X because multiline list items are
    /// already grouped.
    pub(super) fn detect_from_boxes(
        &self,
        page: &[Primitive],
        text_boxes: &[crate::structure::layout::TextBox],
    ) -> Vec<DetectedHint> {
        if text_boxes.is_empty() {
            return Vec::new();
        }

        // Dominant left margin from textboxes.
        let dominant_x = dominant_box_left_margin(text_boxes);

        let mut hints = Vec::new();
        let mut hinted_box_indices = std::collections::HashSet::new();

        // Phase 1: Detect prefix-based list items.
        for (tb_idx, tb) in text_boxes.iter().enumerate() {
            // Get the text of the first line in this box.
            let first_line_text = tb.lines.first().map(|l| l.text()).unwrap_or_default();
            let trimmed = first_line_text.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Check for bullet/number prefix.
            if let Some(list_info) = detect_list_prefix(trimmed) {
                let depth = self.compute_depth(tb.bbox.x, dominant_x);

                // Emit hints for ALL primitive indices covered by this box,
                // but only if the primitive is unhinted.
                let prim_indices = tb.primitive_indices();
                for idx in prim_indices {
                    if idx < page.len() && page[idx].hints.is_empty() {
                        hints.push(DetectedHint {
                            primitive_index: idx,
                            hint: SemanticHint::from_heuristic(
                                HintKind::ListItem {
                                    depth,
                                    ordered: list_info.ordered,
                                    list_group: None,
                                },
                                self.prefix_confidence,
                                "ListDetector",
                            ),
                        });
                    }
                }
                hinted_box_indices.insert(tb_idx);
            }
        }

        // Phase 2: Indent-only detection (if enabled).
        if self.enable_indent_only {
            // Collect unhinted textboxes with non-empty text.
            let unhinted_boxes: Vec<(usize, &crate::structure::layout::TextBox)> = text_boxes
                .iter()
                .enumerate()
                .filter(|(idx, tb)| {
                    !hinted_box_indices.contains(idx) && {
                        let first_line = tb.lines.first().map(|l| l.text()).unwrap_or_default();
                        !first_line.trim().is_empty()
                    }
                })
                .collect();

            if !unhinted_boxes.is_empty() {
                // Group by indent bin (quantized to nearest 0.01).
                let mut indent_groups: std::collections::HashMap<
                    i32,
                    Vec<(usize, &crate::structure::layout::TextBox)>,
                > = std::collections::HashMap::new();
                for &(tb_idx, tb) in &unhinted_boxes {
                    let bin = (tb.bbox.x * 100.0).round() as i32;
                    indent_groups.entry(bin).or_default().push((tb_idx, tb));
                }

                // For each indent bin with 3+ items that exceed min_indent,
                // emit ListItem hints.
                for (bin, group) in indent_groups {
                    if group.len() >= 3 {
                        let bin_x = bin as f32 / 100.0;
                        let indent = bin_x - dominant_x;
                        if indent >= self.min_indent {
                            // Compute depth for this indentation level.
                            let depth = self.compute_depth(bin_x, dominant_x);

                            // Find runs of consecutive items (by textbox index).
                            let mut sorted_group = group.clone();
                            sorted_group.sort_by_key(|(idx, _)| *idx);

                            let mut current_run = vec![sorted_group[0]];
                            for i in 1..sorted_group.len() {
                                let prev_idx = sorted_group[i - 1].0;
                                let curr_idx = sorted_group[i].0;
                                if curr_idx == prev_idx + 1 {
                                    // Consecutive textbox → same run.
                                    current_run.push(sorted_group[i]);
                                } else {
                                    // Gap found.
                                    if current_run.len() >= 3 {
                                        // Emit hints for this run.
                                        for &(_, tb) in &current_run {
                                            let prim_indices = tb.primitive_indices();
                                            for idx in prim_indices {
                                                if idx < page.len() && page[idx].hints.is_empty() {
                                                    hints.push(DetectedHint {
                                                        primitive_index: idx,
                                                        hint: SemanticHint::from_heuristic(
                                                            HintKind::ListItem {
                                                                depth,
                                                                ordered: false,
                                                                list_group: None,
                                                            },
                                                            self.indent_confidence,
                                                            "ListDetector",
                                                        ),
                                                    });
                                                }
                                            }
                                        }
                                    }
                                    current_run = vec![sorted_group[i]];
                                }
                            }
                            // Process the final run.
                            if current_run.len() >= 3 {
                                for &(_, tb) in &current_run {
                                    let prim_indices = tb.primitive_indices();
                                    for idx in prim_indices {
                                        if idx < page.len() && page[idx].hints.is_empty() {
                                            hints.push(DetectedHint {
                                                primitive_index: idx,
                                                hint: SemanticHint::from_heuristic(
                                                    HintKind::ListItem {
                                                        depth,
                                                        ordered: false,
                                                        list_group: None,
                                                    },
                                                    self.indent_confidence,
                                                    "ListDetector",
                                                ),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        hints
    }

    /// Detect list items from raw primitives (fallback when no layout context).
    pub(super) fn detect_from_primitives(&self, page: &[Primitive]) -> Vec<DetectedHint> {
        let text_items: Vec<(usize, &Primitive, &str)> = page
            .iter()
            .enumerate()
            .filter_map(|(i, p)| {
                if !p.hints.is_empty() {
                    return None;
                }
                if let PrimitiveKind::Text { ref content, .. } = p.kind {
                    Some((i, p, content.as_str()))
                } else {
                    None
                }
            })
            .collect();

        if text_items.is_empty() {
            return Vec::new();
        }

        let dominant_x = dominant_left_margin(&text_items);
        let mut hints = Vec::new();
        let mut hinted_indices = std::collections::HashSet::new();

        // Phase 1: Detect prefix-based list items.
        for &(idx, prim, content) in &text_items {
            let trimmed = content.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Some(list_info) = detect_list_prefix(trimmed) {
                let depth = self.compute_depth(prim.bbox.x, dominant_x);

                hints.push(DetectedHint {
                    primitive_index: idx,
                    hint: SemanticHint::from_heuristic(
                        HintKind::ListItem {
                            depth,
                            ordered: list_info.ordered,
                            list_group: None,
                        },
                        self.prefix_confidence,
                        "ListDetector",
                    ),
                });
                hinted_indices.insert(idx);
            }
        }

        // Phase 2: Indent-only detection (if enabled).
        if self.enable_indent_only {
            // Collect unhinted text items with non-empty trimmed content.
            let unhinted_items: Vec<(usize, &Primitive)> = text_items
                .iter()
                .filter_map(|&(idx, prim, content)| {
                    if !hinted_indices.contains(&idx) && !content.trim().is_empty() {
                        Some((idx, prim))
                    } else {
                        None
                    }
                })
                .collect();

            if !unhinted_items.is_empty() {
                // Group by indent bin (quantized to nearest 0.01).
                let mut indent_groups: std::collections::HashMap<i32, Vec<(usize, &Primitive)>> =
                    std::collections::HashMap::new();
                for &(idx, prim) in &unhinted_items {
                    let bin = (prim.bbox.x * 100.0).round() as i32;
                    indent_groups.entry(bin).or_default().push((idx, prim));
                }

                // For each indent bin with 3+ items that exceed min_indent,
                // look for runs of 3+ consecutive items (by primitive index).
                for (bin, group) in indent_groups {
                    if group.len() >= 3 {
                        let bin_x = bin as f32 / 100.0;
                        let indent = bin_x - dominant_x;
                        if indent >= self.min_indent {
                            // Compute depth for this indentation level.
                            let depth = self.compute_depth(bin_x, dominant_x);

                            // Find runs of consecutive items (by primitive index).
                            let mut sorted_group = group.clone();
                            sorted_group.sort_by_key(|(idx, _)| *idx);

                            let mut current_run = vec![sorted_group[0]];
                            for i in 1..sorted_group.len() {
                                let prev_idx = sorted_group[i - 1].0;
                                let curr_idx = sorted_group[i].0;
                                if curr_idx == prev_idx + 1 {
                                    // Consecutive primitive → same run.
                                    current_run.push(sorted_group[i]);
                                } else {
                                    // Gap found.
                                    if current_run.len() >= 3 {
                                        // Emit hints for this run.
                                        for &(idx, _) in &current_run {
                                            hints.push(DetectedHint {
                                                primitive_index: idx,
                                                hint: SemanticHint::from_heuristic(
                                                    HintKind::ListItem {
                                                        depth,
                                                        ordered: false,
                                                        list_group: None,
                                                    },
                                                    self.indent_confidence,
                                                    "ListDetector",
                                                ),
                                            });
                                        }
                                    }
                                    current_run = vec![sorted_group[i]];
                                }
                            }
                            // Process the final run.
                            if current_run.len() >= 3 {
                                for &(idx, _) in &current_run {
                                    hints.push(DetectedHint {
                                        primitive_index: idx,
                                        hint: SemanticHint::from_heuristic(
                                            HintKind::ListItem {
                                                depth,
                                                ordered: false,
                                                list_group: None,
                                            },
                                            self.indent_confidence,
                                            "ListDetector",
                                        ),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        // Phase 3: Definition list detection (if enabled).
        if self.enable_definition_lists {
            // Collect unhinted text items with non-empty trimmed content.
            let unhinted_items: Vec<(usize, &Primitive, &str)> = text_items
                .iter()
                .filter_map(|&(idx, prim, content)| {
                    if !hinted_indices.contains(&idx) && !content.trim().is_empty() {
                        Some((idx, prim, content))
                    } else {
                        None
                    }
                })
                .collect();

            if !unhinted_items.is_empty() {
                // Find runs of 3+ consecutive items that match definition pattern.
                let mut i = 0;
                while i < unhinted_items.len() {
                    let mut current_run = vec![(unhinted_items[i].0, unhinted_items[i].1)];

                    // Look ahead for consecutive items matching definition pattern.
                    let mut j = i + 1;
                    while j < unhinted_items.len() {
                        let (idx, prim, content) = unhinted_items[j];
                        let prev_idx = unhinted_items[j - 1].0;

                        // Must be consecutive and match definition pattern.
                        if idx == prev_idx + 1 && detect_definition_prefix(content).is_some() {
                            current_run.push((idx, prim));
                            j += 1;
                        } else {
                            break;
                        }
                    }

                    // Emit hints for runs of 3+ definition items.
                    if current_run.len() >= 3 {
                        for &(idx, _) in &current_run {
                            hints.push(DetectedHint {
                                primitive_index: idx,
                                hint: SemanticHint::from_heuristic(
                                    HintKind::ListItem {
                                        depth: 0,
                                        ordered: false,
                                        list_group: None,
                                    },
                                    self.indent_confidence,
                                    "ListDetector",
                                ),
                            });
                        }
                    }

                    i = j.max(i + 1);
                }
            }
        }

        hints
    }
}

impl ListDetector {
    /// Assign list group IDs to detected hints.
    ///
    /// Items are grouped by contiguous runs of the same `(depth, ordered)`
    /// signature in primitive-index order. A gap in primitive indices
    /// (i.e., a non-list primitive between two list items) breaks the group.
    pub(super) fn assign_group_ids(hints: &mut [DetectedHint], page: &[Primitive]) {
        if hints.is_empty() {
            return;
        }

        // Sort hints by primitive index to process in reading order.
        hints.sort_by_key(|h| h.primitive_index);

        let mut group_id: u32 = 0;
        let mut prev_index: Option<usize> = None;
        let mut prev_key: Option<(u8, bool)> = None;

        for hint in hints.iter_mut() {
            let current_key = match &hint.hint.kind {
                HintKind::ListItem { depth, ordered, .. } => (*depth, *ordered),
                _ => continue,
            };

            // Check if we should start a new group:
            // 1. First item always starts a new group.
            // 2. Different (depth, ordered) signature → new group.
            // 3. Non-consecutive primitive indices with a non-empty text
            //    primitive between them → new group (paragraph break).
            let new_group = match (prev_index, prev_key) {
                (None, _) => true,
                (_, None) => true,
                (Some(pi), Some(pk)) => {
                    if pk != current_key {
                        true
                    } else {
                        // Check if there's a non-list text primitive between
                        // the previous item and this one.
                        has_text_gap(page, pi, hint.primitive_index)
                    }
                }
            };

            if new_group && prev_index.is_some() {
                group_id += 1;
            }

            // Update the list_group field.
            if let HintKind::ListItem {
                ref mut list_group, ..
            } = hint.hint.kind
            {
                *list_group = Some(group_id);
            }

            prev_index = Some(hint.primitive_index);
            prev_key = Some(current_key);
        }
    }

    /// Compute nesting depth from X indentation.
    pub(super) fn compute_depth(&self, x: f32, dominant_x: f32) -> u8 {
        let indent = x - dominant_x;
        if indent < self.min_indent {
            return 0;
        }
        let raw_depth = (indent / self.indent_step) as u8;
        raw_depth.min(5) // Cap at depth 5
    }
}
