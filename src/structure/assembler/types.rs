use std::collections::BTreeMap;

use crate::model::BoundingBox;
use crate::model::provenance::StructureSource;

#[derive(Debug)]
pub(super) struct OpenSection {
    pub(super) level: u8,
    pub(super) title: Option<String>,
    pub(super) children: Vec<crate::model::DocumentNode>,
    pub(super) start_page: u32,
    pub(super) bbox: Option<BoundingBox>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum TextBlockKind {
    Paragraph,
    Heading(u8),
    CodeBlock,
    BlockQuote,
}

#[derive(Debug)]
pub(super) struct TextAccumulator {
    pub(super) kind: TextBlockKind,
    pub(super) text: String,
    pub(super) bbox: BoundingBox,
    pub(super) start_page: u32,
    pub(super) end_page: u32,
    pub(super) min_confidence: f32,
    pub(super) structure_source: StructureSource,
    pub(super) metadata: Vec<(String, String)>,
    pub(super) last_font_size: Option<f32>,
}

#[derive(Debug)]
pub(super) struct OpenTable {
    pub(super) cells: BTreeMap<u32, BTreeMap<u32, CellAccumulator>>,
    pub(super) start_page: u32,
    pub(super) end_page: u32,
    pub(super) bbox: Option<BoundingBox>,
    pub(super) min_confidence: f32,
    pub(super) repeated_header_pages: Vec<u32>,
    pub(super) continuation_row_base: Option<u32>,
}

#[derive(Debug)]
pub(super) struct CellAccumulator {
    pub(super) text: String,
    pub(super) rowspan: u32,
    pub(super) colspan: u32,
    pub(super) is_header: bool,
    pub(super) bbox: BoundingBox,
    pub(super) page: u32,
    pub(super) confidence: f32,
    pub(super) structure_source: StructureSource,
}

#[derive(Debug)]
pub(super) struct OpenList {
    pub(super) ordered: bool,
    pub(super) items: Vec<ListItemAcc>,
    pub(super) start_page: u32,
    pub(super) end_page: u32,
    pub(super) bbox: Option<BoundingBox>,
    pub(super) min_confidence: f32,
    /// Group identifier carried by the most recent `ListItem` hint accepted
    /// into this list. Used to detect when a new list group starts back-to-back
    /// without an intervening role transition (e.g. bullets followed directly
    /// by a numbered list emitted by `ListDetector` with different `list_group`
    /// values). `None` when the current or previous hint did not provide a
    /// group identifier, in which case we fall back to the legacy behaviour
    /// (only close on ordered-flag change or external role transition).
    pub(super) current_group: Option<u32>,
}

#[derive(Debug)]
pub(super) struct ListItemAcc {
    pub(super) text: String,
    pub(super) depth: u8,
    pub(super) bbox: BoundingBox,
    pub(super) page: u32,
    pub(super) confidence: f32,
    pub(super) structure_source: StructureSource,
}

#[derive(Debug)]
pub(super) enum PrimaryRole<'a> {
    SectionStart {
        title: &'a Option<String>,
    },
    SectionEnd,
    Heading {
        level: u8,
        confidence: f32,
    },
    Paragraph {
        confidence: f32,
    },
    TableCell {
        row: u32,
        col: u32,
        rowspan: u32,
        colspan: u32,
        is_header: bool,
        confidence: f32,
    },
    ListItem {
        depth: u8,
        ordered: bool,
        confidence: f32,
        list_group: Option<u32>,
    },
    PageHeader {
        confidence: f32,
    },
    PageFooter {
        confidence: f32,
    },
    CodeBlock {
        confidence: f32,
    },
    BlockQuote {
        confidence: f32,
    },
    Footnote {
        id: &'a str,
        confidence: f32,
    },
    DocumentTitle,
    Sidebar {
        confidence: f32,
    },
    ImageBare,
    DecorationBare,
    TextBare,
    ContainedInTable {
        row: u32,
        col: u32,
        confidence: f32,
    },
    TableCellContinuation {
        row: u32,
        col: u32,
        rowspan: u32,
        colspan: u32,
        confidence: f32,
    },
}
