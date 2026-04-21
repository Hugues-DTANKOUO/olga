use super::*;
use crate::model::{HintKind, PrimitiveKind};

impl Assembler {
    /// Extract the primary structural role from a primitive's hints.
    ///
    /// Uses a priority-based system: all hints are scanned in a single pass and
    /// the highest-priority candidate wins.
    pub(super) fn classify(prim: &Primitive) -> PrimaryRole<'_> {
        let mut best: Option<(u8, PrimaryRole<'_>)> = None;

        macro_rules! candidate {
            ($best:expr_2021, $tier:expr_2021, $role:expr_2021) => {
                match &$best {
                    Some((current_tier, _)) if *current_tier <= $tier => {}
                    _ => {
                        $best = Some(($tier, $role));
                    }
                }
            };
        }

        for hint in &prim.hints {
            match &hint.kind {
                HintKind::SectionStart { title } => {
                    candidate!(best, 0, PrimaryRole::SectionStart { title });
                }
                HintKind::SectionEnd => {
                    candidate!(best, 0, PrimaryRole::SectionEnd);
                }
                HintKind::TableCell {
                    row,
                    col,
                    rowspan,
                    colspan,
                } => {
                    candidate!(
                        best,
                        1,
                        PrimaryRole::TableCell {
                            row: *row,
                            col: *col,
                            rowspan: *rowspan,
                            colspan: *colspan,
                            is_header: false,
                            confidence: hint.confidence,
                        }
                    );
                }
                HintKind::TableHeader { col } => {
                    candidate!(
                        best,
                        1,
                        PrimaryRole::TableCell {
                            row: 0,
                            col: *col,
                            rowspan: 1,
                            colspan: 1,
                            is_header: true,
                            confidence: hint.confidence,
                        }
                    );
                }
                HintKind::TableCellContinuation {
                    row,
                    col,
                    rowspan,
                    colspan,
                } => {
                    candidate!(
                        best,
                        1,
                        PrimaryRole::TableCellContinuation {
                            row: *row,
                            col: *col,
                            rowspan: *rowspan,
                            colspan: *colspan,
                            confidence: hint.confidence,
                        }
                    );
                }
                HintKind::ListItem {
                    depth,
                    ordered,
                    list_group,
                } => {
                    candidate!(
                        best,
                        2,
                        PrimaryRole::ListItem {
                            depth: *depth,
                            ordered: *ordered,
                            confidence: hint.confidence,
                            list_group: *list_group,
                        }
                    );
                }
                HintKind::PageHeader => {
                    candidate!(
                        best,
                        3,
                        PrimaryRole::PageHeader {
                            confidence: hint.confidence,
                        }
                    );
                }
                HintKind::PageFooter => {
                    candidate!(
                        best,
                        3,
                        PrimaryRole::PageFooter {
                            confidence: hint.confidence,
                        }
                    );
                }
                HintKind::Heading { level } => {
                    candidate!(
                        best,
                        4,
                        PrimaryRole::Heading {
                            level: *level,
                            confidence: hint.confidence,
                        }
                    );
                }
                HintKind::Paragraph => {
                    candidate!(
                        best,
                        5,
                        PrimaryRole::Paragraph {
                            confidence: hint.confidence,
                        }
                    );
                }
                HintKind::CodeBlock => {
                    candidate!(
                        best,
                        6,
                        PrimaryRole::CodeBlock {
                            confidence: hint.confidence,
                        }
                    );
                }
                HintKind::BlockQuote => {
                    candidate!(
                        best,
                        6,
                        PrimaryRole::BlockQuote {
                            confidence: hint.confidence,
                        }
                    );
                }
                HintKind::Footnote { id } => {
                    candidate!(
                        best,
                        6,
                        PrimaryRole::Footnote {
                            id,
                            confidence: hint.confidence,
                        }
                    );
                }
                HintKind::DocumentTitle => {
                    candidate!(best, 6, PrimaryRole::DocumentTitle);
                }
                HintKind::ContainedInTableCell { row, col, .. }
                | HintKind::FlattenedFromTableCell { row, col, .. } => {
                    candidate!(
                        best,
                        7,
                        PrimaryRole::ContainedInTable {
                            row: *row,
                            col: *col,
                            confidence: hint.confidence,
                        }
                    );
                }
                HintKind::Sidebar => {
                    candidate!(
                        best,
                        8,
                        PrimaryRole::Sidebar {
                            confidence: hint.confidence,
                        }
                    );
                }
                HintKind::Link { .. }
                | HintKind::Emphasis
                | HintKind::Strong
                | HintKind::ExpandedText { .. } => {}
            }
        }

        if let Some((_, role)) = best {
            return role;
        }

        match &prim.kind {
            PrimitiveKind::Image { .. } => PrimaryRole::ImageBare,
            PrimitiveKind::Text { .. } => PrimaryRole::TextBare,
            PrimitiveKind::Line { .. } | PrimitiveKind::Rectangle { .. } => {
                PrimaryRole::DecorationBare
            }
        }
    }
}
