use crate::model::{HintKind, Primitive, PrimitiveKind};

/// Logical "story" the block came from.
///
/// DOCX (and other OOXML-style formats) emit several independent narrative
/// streams into one document: the linear body text, plus chrome stories for
/// page headers, page footers, footnotes/endnotes, and sidebars/comments.
/// Each story is its own coordinate space — a table inside a footer is a
/// distinct table from any table in the body, even though both wear
/// `TableHeader { col: 0 }` hints with `row = 0`.
///
/// We carry the story tag through to the renderer so a contiguous run of
/// table cells stops at any story boundary. Without this, the body table's
/// row 0 and the footer table's row 0 land in the same grid and the footer
/// columns overwrite the body columns (this was the visible defect on
/// memo.docx — "Confidential" / "Page 1 of 1" overwriting
/// "Expense category" / "Up to $500").
///
/// Apache POI's `XWPFWordExtractor`, python-docx's `Section.header/.footer`
/// object graph, and Mammoth's "drop chrome" default all keep these
/// streams in separate output regions. Tracking the story here is olga's
/// equivalent: same data, segregated rendering.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum BlockStory {
    Body,
    Header,
    Footer,
    Footnote,
    Sidebar,
}

/// A text block extracted from a primitive, with semantic info from hints.
pub(super) struct Block {
    pub(super) text: String,
    pub(super) is_bold: bool,
    pub(super) is_italic: bool,
    pub(super) heading_level: u8,
    pub(super) table_cell: Option<(u32, u32)>,
    pub(super) is_list_item: bool,
    pub(super) list_ordered: bool,
    pub(super) link_url: Option<String>,
    pub(super) story: BlockStory,
}

// ---------------------------------------------------------------------------
// Block extraction
// ---------------------------------------------------------------------------

pub(super) fn extract_blocks(prims: &[&Primitive]) -> Vec<Block> {
    let mut blocks = Vec::new();

    for prim in prims {
        let (content, is_bold, is_italic) = match &prim.kind {
            PrimitiveKind::Text {
                content,
                is_bold,
                is_italic,
                ..
            } => {
                if content.trim().is_empty() {
                    continue;
                }
                (content.trim().to_string(), *is_bold, *is_italic)
            }
            _ => continue,
        };

        let mut heading_level: u8 = 0;
        let mut table_cell: Option<(u32, u32)> = None;
        let mut is_list_item = false;
        let mut list_ordered = false;
        let mut link_url: Option<String> = None;
        // Default story is Body; a chrome hint upgrades it. If a primitive
        // carries several chrome hints we pick the most specific in
        // priority order (Footnote > Sidebar > Footer > Header). This order
        // never matters in practice — `attach_docx_media_structural_hints`
        // only attaches one chrome hint per primitive — but the tie-break
        // is deterministic anyway so the renderer can't flip on it.
        let mut story = BlockStory::Body;

        for hint in &prim.hints {
            match &hint.kind {
                HintKind::Heading { level } => heading_level = *level,
                HintKind::TableCell { row, col, .. } => table_cell = Some((*row, *col)),
                HintKind::TableHeader { col } => table_cell = Some((0, *col)),
                HintKind::ListItem { ordered, .. } => {
                    is_list_item = true;
                    list_ordered = *ordered;
                }
                HintKind::Link { url } => link_url = Some(url.clone()),
                HintKind::PageHeader if story == BlockStory::Body => {
                    story = BlockStory::Header;
                }
                HintKind::PageFooter if matches!(story, BlockStory::Body | BlockStory::Header) => {
                    story = BlockStory::Footer;
                }
                HintKind::Sidebar if !matches!(story, BlockStory::Footnote) => {
                    story = BlockStory::Sidebar;
                }
                HintKind::Footnote { .. } => {
                    story = BlockStory::Footnote;
                }
                _ => {}
            }
        }

        blocks.push(Block {
            text: content,
            is_bold,
            is_italic,
            heading_level,
            table_cell,
            is_list_item,
            list_ordered,
            link_url,
            story,
        });
    }

    blocks
}
