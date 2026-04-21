use super::*;

impl Assembler {
    pub(super) fn open_section(&mut self, level: u8, title: Option<String>, page: u32) {
        while self.section_stack.last().is_some_and(|s| s.level >= level) {
            self.close_top_section();
        }

        self.section_stack.push(OpenSection {
            level,
            title,
            children: Vec::new(),
            start_page: page,
            bbox: None,
        });
    }

    pub(super) fn close_top_section(&mut self) {
        let section = match self.section_stack.pop() {
            Some(s) => s,
            None => return,
        };

        let bbox = section
            .bbox
            .or_else(|| self.merged_bbox_opt(&section.children))
            .unwrap_or(BoundingBox::new(0.0, 0.0, 1.0, 1.0));

        let max_page = section
            .children
            .iter()
            .map(|c| c.source_pages.end)
            .max()
            .unwrap_or(section.start_page + 1);

        let mut node = DocumentNode::new(
            NodeKind::Section {
                level: section.level,
            },
            bbox,
            1.0,
            StructureSource::HintAssembled,
            section.start_page..max_page,
        );
        node.children = section.children;

        if let Some(title) = &section.title
            && !title.is_empty()
        {
            node.metadata.insert("title".to_string(), title.clone());
        }

        self.emit_node(node);
    }

    pub(super) fn infer_section_level(&self) -> u8 {
        (self.section_stack.len() as u8).saturating_add(1)
    }
}
