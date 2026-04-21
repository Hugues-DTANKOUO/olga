use super::*;

impl Assembler {
    pub(super) fn build_vline_index(
        flush: &PageFlush,
    ) -> std::collections::HashMap<usize, (usize, bool)> {
        let mut index = std::collections::HashMap::new();
        if let Some(ref lines) = flush.visual_lines {
            for (line_idx, line) in lines.iter().enumerate() {
                let multi = line.spans.len() > 1;
                for span in &line.spans {
                    index.insert(span.prim_index, (line_idx, multi));
                }
            }
        }
        index
    }

    pub(super) fn emit_aligned_line(
        &mut self,
        line: &crate::structure::visual_lines::VisualLine,
        primitives: &[Primitive],
        page: u32,
    ) {
        self.flush_text_acc();
        self.close_table();
        self.close_list();

        let spans: Vec<crate::model::InlineSpan> = line
            .spans
            .iter()
            .map(|s| {
                let (is_bold, is_italic, link_url) = if s.prim_index < primitives.len() {
                    Self::extract_inline_format(&primitives[s.prim_index])
                } else {
                    (false, false, None)
                };
                crate::model::InlineSpan {
                    col_start: s.col_start,
                    text: s.text.clone(),
                    is_bold,
                    is_italic,
                    link_url,
                }
            })
            .collect();

        if spans.is_empty() {
            return;
        }

        let node = DocumentNode::new(
            NodeKind::AlignedLine { spans },
            line.bbox,
            0.9,
            StructureSource::HintAssembled,
            page..page + 1,
        );
        self.emit_node(node);
    }

    pub(super) fn emit_merged_heading(
        &mut self,
        line: &crate::structure::visual_lines::VisualLine,
        primitives: &[Primitive],
        page: u32,
    ) {
        self.flush_text_acc();
        self.close_table();
        self.close_list();

        if has_wide_gap(&line.spans, 0.10) {
            self.emit_aligned_line(line, primitives, page);
            return;
        }

        let valid_spans: Vec<_> = line
            .spans
            .iter()
            .filter(|s| s.prim_index < primitives.len())
            .collect();
        let heading_count = valid_spans
            .iter()
            .filter(|s| {
                matches!(
                    Self::classify(&primitives[s.prim_index]),
                    PrimaryRole::Heading { .. }
                )
            })
            .count();

        let non_heading_count = valid_spans.len() - heading_count;
        let heading_char_count: usize = valid_spans
            .iter()
            .filter(|s| {
                matches!(
                    Self::classify(&primitives[s.prim_index]),
                    PrimaryRole::Heading { .. }
                )
            })
            .map(|s| s.text.chars().count())
            .sum();
        let total_char_count: usize = valid_spans.iter().map(|s| s.text.chars().count()).sum();

        let full_text: String = valid_spans
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let non_heading_text: String = valid_spans
            .iter()
            .filter(|s| {
                !matches!(
                    Self::classify(&primitives[s.prim_index]),
                    PrimaryRole::Heading { .. }
                )
            })
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        let has_label_separator = full_text.contains(':') || non_heading_text.contains(':');
        let heading_is_minority =
            total_char_count > 0 && (heading_char_count as f32 / total_char_count as f32) < 0.5;

        if has_label_separator && heading_is_minority && non_heading_count >= 2 {
            self.emit_aligned_line(line, primitives, page);
            return;
        }

        let mut level = 1u8;
        for span in &line.spans {
            if span.prim_index < primitives.len()
                && let PrimaryRole::Heading { level: l, .. } =
                    Self::classify(&primitives[span.prim_index])
            {
                level = l;
                break;
            }
        }

        let merged_text: String = line
            .spans
            .iter()
            .filter(|s| s.prim_index < primitives.len())
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        let trimmed = merged_text.trim();
        if trimmed.is_empty() {
            return;
        }

        let node = DocumentNode::new(
            NodeKind::Heading {
                level,
                text: trimmed.to_string(),
            },
            line.bbox,
            0.9,
            StructureSource::HintAssembled,
            page..page + 1,
        );
        self.emit_node(node);
    }
}

pub(super) fn heading_text_looks_valid(text: &str) -> bool {
    let trimmed = text.trim();
    let char_count = trimmed.chars().count();

    if char_count == 0 {
        return false;
    }
    if char_count > 100 {
        return false;
    }
    if trimmed.ends_with('?') || trimmed.ends_with('!') {
        return false;
    }
    if let Some(before_dot) = trimmed.strip_suffix('.')
        && !before_dot.ends_with(|c: char| c.is_ascii_digit())
    {
        return false;
    }

    let bytes = trimmed.as_bytes();
    for i in 0..bytes.len().saturating_sub(2) {
        if bytes[i] == b'.' && bytes[i + 1] == b' ' && bytes[i + 2].is_ascii_uppercase() {
            if i > 0 && bytes[i - 1].is_ascii_uppercase() && (i < 2 || bytes[i - 2] == b'.') {
                continue;
            }
            return false;
        }
    }

    if char_count > 50 && trimmed.contains(',') {
        return false;
    }

    true
}

pub(super) fn has_wide_gap(
    spans: &[crate::structure::visual_lines::AlignedSpan],
    threshold: f32,
) -> bool {
    if spans.len() < 2 {
        return false;
    }
    for i in 0..spans.len() - 1 {
        let right_edge = spans[i].bbox.x + spans[i].bbox.width;
        let next_left = spans[i + 1].bbox.x;
        if next_left - right_edge > threshold {
            return true;
        }
    }
    false
}
