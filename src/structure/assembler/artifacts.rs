use super::*;

impl Assembler {
    pub(super) fn handle_artifact(
        &mut self,
        prim: &Primitive,
        page: u32,
        is_header: bool,
        confidence: f32,
    ) {
        let text = Self::text_of(prim);
        let kind = if is_header {
            NodeKind::PageHeader { text }
        } else {
            NodeKind::PageFooter { text }
        };
        let structure_source = Self::structure_source_for(prim);
        let node = DocumentNode::new(
            kind,
            prim.bbox,
            confidence,
            structure_source,
            page..page + 1,
        );

        if self.config.strip_artifacts {
            self.artifacts.push(node);
        } else {
            self.emit_node(node);
        }
    }

    pub(super) fn emit_node(&mut self, node: DocumentNode) {
        if let Some(section) = self.section_stack.last_mut() {
            section.bbox = Some(match section.bbox {
                Some(b) => b.merge(&node.bbox),
                None => node.bbox,
            });
            section.children.push(node);
        } else {
            self.output.push(node);
        }
    }
}
