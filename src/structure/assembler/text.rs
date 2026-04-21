use super::*;
use crate::model::{HintKind, PrimitiveKind};

impl Assembler {
    pub(super) fn accumulate_text(
        &mut self,
        prim: &Primitive,
        page: u32,
        kind: TextBlockKind,
        confidence: f32,
    ) {
        let text = Self::text_of(prim);
        let inline_meta = Self::collect_inline_metadata(prim);

        if let Some(ref mut acc) = self.text_acc
            && acc.kind == kind
        {
            let acc_bottom = acc.bbox.y + acc.bbox.height;
            let prim_top = prim.bbox.y;
            let acc_height = acc.bbox.height.max(0.005);
            let gap = prim_top - acc_bottom;

            let gap_break = gap > acc_height * 1.8 && gap > 0.015;
            let prim_font_size = Self::font_size_of(prim);
            let font_break =
                if let (Some(acc_size), Some(prim_size)) = (acc.last_font_size, prim_font_size) {
                    let ratio = if acc_size > prim_size {
                        acc_size / prim_size.max(0.1)
                    } else {
                        prim_size / acc_size.max(0.1)
                    };
                    ratio > 1.10
                } else {
                    false
                };

            if gap_break || font_break {
                self.flush_text_acc();
                let structure_source = Self::structure_source_for(prim);
                self.text_acc = Some(TextAccumulator {
                    kind,
                    text,
                    bbox: prim.bbox,
                    start_page: page,
                    end_page: page,
                    min_confidence: confidence,
                    structure_source,
                    metadata: inline_meta,
                    last_font_size: Self::font_size_of(prim),
                });
                return;
            }

            if !acc.text.is_empty() && !text.is_empty() {
                match kind {
                    TextBlockKind::CodeBlock => acc.text.push('\n'),
                    _ => {
                        let ends_ws = acc.text.chars().last().is_some_and(|c| c.is_whitespace());
                        let starts_ws = text.chars().next().is_some_and(|c| c.is_whitespace());
                        if !ends_ws && !starts_ws {
                            acc.text.push(' ');
                        }
                    }
                }
            }
            acc.text.push_str(&text);
            acc.bbox = acc.bbox.merge(&prim.bbox);
            acc.end_page = page;
            acc.min_confidence = acc.min_confidence.min(confidence);
            acc.metadata.extend(inline_meta);
            acc.last_font_size = Self::font_size_of(prim).or(acc.last_font_size);
            return;
        }

        self.flush_text_acc();

        let structure_source = Self::structure_source_for(prim);
        self.text_acc = Some(TextAccumulator {
            kind,
            text,
            bbox: prim.bbox,
            start_page: page,
            end_page: page,
            min_confidence: confidence,
            structure_source,
            metadata: inline_meta,
            last_font_size: Self::font_size_of(prim),
        });
    }

    pub(super) fn flush_text_acc(&mut self) {
        let acc = match self.text_acc.take() {
            Some(a) => a,
            None => return,
        };

        if acc.text.is_empty() {
            return;
        }

        let pages = acc.start_page..acc.end_page + 1;
        let mut node = match acc.kind {
            TextBlockKind::Paragraph => DocumentNode::new(
                NodeKind::Paragraph { text: acc.text },
                acc.bbox,
                acc.min_confidence,
                acc.structure_source,
                pages,
            ),
            TextBlockKind::Heading(level) => DocumentNode::new(
                NodeKind::Heading {
                    level,
                    text: acc.text,
                },
                acc.bbox,
                acc.min_confidence,
                acc.structure_source,
                pages,
            ),
            TextBlockKind::CodeBlock => DocumentNode::new(
                NodeKind::CodeBlock {
                    language: None,
                    text: acc.text,
                },
                acc.bbox,
                acc.min_confidence,
                acc.structure_source,
                pages,
            ),
            TextBlockKind::BlockQuote => DocumentNode::new(
                NodeKind::BlockQuote { text: acc.text },
                acc.bbox,
                acc.min_confidence,
                acc.structure_source,
                pages,
            ),
        };

        for (k, v) in acc.metadata {
            node.metadata.entry(k).or_insert(v);
        }

        self.emit_node(node);
    }

    pub(super) fn emit_image_node(&mut self, prim: &Primitive, page: u32) {
        if let PrimitiveKind::Image {
            format,
            data,
            alt_text,
        } = &prim.kind
        {
            let structure_source = Self::structure_source_for(prim);
            let mut node = DocumentNode::new(
                NodeKind::Image {
                    alt_text: alt_text.clone(),
                    format: format.to_string(),
                },
                prim.bbox,
                1.0,
                structure_source,
                page..page + 1,
            );
            if !data.is_empty() {
                let image_hex = data
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<String>();
                node.metadata
                    .insert("image_data_hex".to_string(), image_hex);
            }
            Self::propagate_inline_metadata(prim, &mut node);
            self.emit_node(node);
        }
    }

    pub(super) fn collect_inline_metadata(prim: &Primitive) -> Vec<(String, String)> {
        let mut meta = Vec::new();
        for hint in &prim.hints {
            match &hint.kind {
                HintKind::Link { url } => {
                    meta.push(("link_url".to_string(), url.clone()));
                }
                HintKind::ExpandedText { text } => {
                    meta.push(("expanded_text".to_string(), text.clone()));
                }
                HintKind::Emphasis => {
                    meta.push(("emphasis".to_string(), "italic".to_string()));
                }
                HintKind::Strong => {
                    meta.push(("emphasis".to_string(), "bold".to_string()));
                }
                _ => {}
            }
        }
        meta
    }

    pub(super) fn propagate_inline_metadata(prim: &Primitive, node: &mut DocumentNode) {
        for hint in &prim.hints {
            match &hint.kind {
                HintKind::Link { url } => {
                    node.metadata
                        .entry("link_url".to_string())
                        .or_insert_with(|| url.clone());
                }
                HintKind::ExpandedText { text } => {
                    node.metadata
                        .entry("expanded_text".to_string())
                        .or_insert_with(|| text.clone());
                }
                HintKind::Emphasis => {
                    node.metadata
                        .entry("emphasis".to_string())
                        .or_insert_with(|| "italic".to_string());
                }
                HintKind::Strong => {
                    node.metadata
                        .entry("emphasis".to_string())
                        .or_insert_with(|| "bold".to_string());
                }
                _ => {}
            }
        }
    }

    pub(super) fn extract_inline_format(prim: &Primitive) -> (bool, bool, Option<String>) {
        let mut is_bold = false;
        let mut is_italic = false;
        let mut link_url = None;

        if let PrimitiveKind::Text {
            is_bold: b,
            is_italic: i,
            ..
        } = &prim.kind
        {
            is_bold = *b;
            is_italic = *i;
        }

        for hint in &prim.hints {
            match &hint.kind {
                HintKind::Strong => is_bold = true,
                HintKind::Emphasis => is_italic = true,
                HintKind::Link { url } => link_url = Some(url.clone()),
                _ => {}
            }
        }

        (is_bold, is_italic, link_url)
    }
}
