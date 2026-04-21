use super::types::ListItemAcc;
use super::*;

impl Assembler {
    pub(super) fn route_to_list(
        &mut self,
        prim: &Primitive,
        page: u32,
        depth: u8,
        ordered: bool,
        confidence: f32,
        list_group: Option<u32>,
    ) {
        // Close the current list and open a fresh one whenever the incoming
        // item signals a semantic grouping break:
        //
        //   1. `ordered` flag changes (bullets → numbers, or vice versa) —
        //      a single `List` node cannot carry both orderings.
        //   2. `list_group` is `Some(new)` while the open list was opened
        //      with `Some(old)` and `new != old`. This is the explicit signal
        //      emitted by `ListDetector` when consecutive items belong to
        //      distinct lists that happen to share an ordering flag (e.g.
        //      two bullet lists separated by a paragraph where the paragraph
        //      didn't make it into this route).
        //
        // Missing group ids (`None`) never force a split, preserving the
        // pre-existing behaviour for hint sources that don't populate
        // `list_group` (format decoders, legacy detectors, hand-crafted test
        // fixtures).
        if let Some(existing) = self.open_list.as_ref() {
            let ordered_changed = existing.ordered != ordered;
            let group_changed = matches!(
                (existing.current_group, list_group),
                (Some(a), Some(b)) if a != b
            );
            if ordered_changed || group_changed {
                self.close_list();
            }
        }

        let text = Self::text_of(prim);
        let structure_source = Self::structure_source_for(prim);

        let list = self.open_list.get_or_insert_with(|| OpenList {
            ordered,
            items: Vec::new(),
            start_page: page,
            end_page: page,
            bbox: None,
            min_confidence: confidence,
            current_group: list_group,
        });

        list.end_page = page;
        list.min_confidence = list.min_confidence.min(confidence);
        list.bbox = Some(match list.bbox {
            Some(b) => b.merge(&prim.bbox),
            None => prim.bbox,
        });
        // Remember the latest group id we saw so that the next item in this
        // list can be compared against it. Only overwrite when the incoming
        // hint actually carried a group id; otherwise keep whatever we already
        // knew (hint sources that don't populate `list_group` shouldn't erase
        // the group history of detector-emitted items).
        if list_group.is_some() {
            list.current_group = list_group;
        }

        let starts_new_item = if list.items.is_empty() || list_group.is_none() {
            true
        } else {
            Self::text_starts_list_marker(&text)
                || list.items.last().is_some_and(|prev| prev.depth != depth)
        };

        if starts_new_item {
            list.items.push(ListItemAcc {
                text,
                depth,
                bbox: prim.bbox,
                page,
                confidence,
                structure_source,
            });
        } else if let Some(last) = list.items.last_mut() {
            if !text.is_empty() {
                if !last.text.is_empty() {
                    last.text.push(' ');
                }
                last.text.push_str(&text);
            }
            last.bbox = last.bbox.merge(&prim.bbox);
            last.confidence = last.confidence.min(confidence);
        }
    }

    pub(super) fn text_starts_list_marker(text: &str) -> bool {
        let trimmed = text.trim_start();
        if trimmed.is_empty() {
            return false;
        }

        const BULLETS: &[char] = &[
            '•', '–', '—', '◦', '▪', '‣', '○', '●', '■', '□', '▸', '▹', '►',
        ];

        if let Some(first_char) = trimmed.chars().next()
            && BULLETS.contains(&first_char)
        {
            let rest = &trimmed[first_char.len_utf8()..];
            if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
                return true;
            }
        }

        if (trimmed.starts_with("- ") || trimmed.starts_with("* ")) && trimmed.len() > 2 {
            return true;
        }

        let bytes = trimmed.as_bytes();
        if bytes[0] == b'('
            && let Some(close) = trimmed.find(')')
            && (2..=5).contains(&close)
        {
            let inner = &trimmed[1..close];
            let is_list_marker = if inner.len() == 1 {
                inner.as_bytes()[0].is_ascii_alphanumeric()
            } else if inner.len() <= 2 && inner.bytes().all(|b| b.is_ascii_digit()) {
                true
            } else {
                matches!(
                    inner.to_ascii_lowercase().as_str(),
                    "ii" | "iii" | "iv" | "v" | "vi" | "vii" | "viii" | "ix" | "x" | "xi" | "xii"
                )
            };
            if is_list_marker {
                let rest = &trimmed[close + 1..];
                return rest.is_empty() || rest.starts_with(' ');
            }
        } else {
            for (i, &b) in bytes.iter().enumerate().take(7) {
                if b == b'.' || b == b')' {
                    if i > 0 && bytes[..i].iter().all(|b| b.is_ascii_alphanumeric()) {
                        let rest = &trimmed[i + 1..];
                        return rest.is_empty() || rest.starts_with(' ');
                    }
                    return false;
                }
                if !b.is_ascii_alphanumeric() {
                    break;
                }
            }
        }

        false
    }

    pub(super) fn close_list(&mut self) {
        let list = match self.open_list.take() {
            Some(l) => l,
            None => return,
        };

        if list.items.is_empty() {
            return;
        }

        let bbox = list.bbox.unwrap_or(BoundingBox::new(0.0, 0.0, 1.0, 1.0));
        let pages = list.start_page..list.end_page + 1;

        let list_source = list
            .items
            .first()
            .map(|i| i.structure_source.clone())
            .unwrap_or(StructureSource::HintAssembled);

        let mut list_node = DocumentNode::new(
            NodeKind::List {
                ordered: list.ordered,
            },
            bbox,
            list.min_confidence,
            list_source,
            pages,
        );

        for item in &list.items {
            let mut item_node = DocumentNode::new(
                NodeKind::ListItem {
                    text: item.text.clone(),
                },
                item.bbox,
                item.confidence,
                item.structure_source.clone(),
                item.page..item.page + 1,
            );
            if item.depth > 0 {
                item_node
                    .metadata
                    .insert("depth".to_string(), item.depth.to_string());
            }
            list_node.add_child(item_node);
        }

        self.emit_node(list_node);
    }
}
