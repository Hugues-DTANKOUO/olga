use super::*;

impl Assembler {
    pub(super) fn on_page_boundary(&mut self, next_page_prims: &[Primitive], next_page: u32) {
        let first_role = self.first_content_role(next_page_prims);

        if self.open_table.is_some() {
            match &first_role {
                Some(
                    PrimaryRole::TableCell { .. }
                    | PrimaryRole::TableCellContinuation { .. }
                    | PrimaryRole::ContainedInTable { .. },
                ) => {
                    self.detect_and_mark_repeated_header(next_page_prims, next_page);
                    if let Some(ref mut table) = self.open_table {
                        table.continuation_row_base = None;
                    }
                }
                _ => {
                    self.close_table();
                }
            }
        }

        if let Some(ref open_list) = self.open_list {
            match &first_role {
                Some(PrimaryRole::ListItem { depth, .. }) => {
                    let max_depth = open_list
                        .items
                        .iter()
                        .map(|item| item.depth)
                        .max()
                        .unwrap_or(0);
                    if *depth > max_depth + 1 {
                        self.warnings.push(Warning {
                            kind: WarningKind::HeuristicInference,
                            message: format!(
                                "List depth jumped from {} to {} at page boundary (page {})",
                                max_depth, depth, next_page
                            ),
                            page: Some(next_page),
                        });
                    }
                }
                _ => {
                    self.close_list();
                }
            }
        }

        if self.text_acc.is_some() && !self.should_merge_paragraph(next_page_prims) {
            self.flush_text_acc();
        }
    }

    pub(super) fn first_content_role<'b>(&self, prims: &'b [Primitive]) -> Option<PrimaryRole<'b>> {
        for prim in prims {
            let role = Self::classify(prim);
            match role {
                PrimaryRole::PageHeader { .. }
                | PrimaryRole::PageFooter { .. }
                | PrimaryRole::DecorationBare => continue,
                _ => return Some(role),
            }
        }
        None
    }

    pub(super) fn should_merge_paragraph(&self, next_page_prims: &[Primitive]) -> bool {
        let acc = match &self.text_acc {
            Some(a) if a.kind == TextBlockKind::Paragraph => a,
            _ => return false,
        };
        if acc.text.is_empty() {
            return false;
        }

        for prim in next_page_prims {
            let role = Self::classify(prim);
            match role {
                PrimaryRole::PageHeader { .. }
                | PrimaryRole::PageFooter { .. }
                | PrimaryRole::DecorationBare => continue,
                PrimaryRole::TextBare => {
                    let text = Self::text_of(prim);
                    if Self::is_continuation_text(&text) {
                        return true;
                    }
                    return self.geometry_matches_paragraph(prim);
                }
                _ => {
                    return false;
                }
            }
        }
        false
    }

    pub(super) fn geometry_matches_paragraph(&self, prim: &Primitive) -> bool {
        let acc = match &self.text_acc {
            Some(a) => a,
            None => return false,
        };

        let trimmed = acc.text.trim_end();
        if let Some(last_char) = trimmed.chars().last() {
            if matches!(last_char, '!' | '?') {
                return false;
            }
            if last_char == '.' {
                let last_word_len = trimmed
                    .rsplit(|c: char| c.is_whitespace())
                    .next()
                    .map(|w| w.len())
                    .unwrap_or(0);
                if last_word_len > 4 {
                    return false;
                }
            }
        }

        let acc_x = acc.bbox.x;
        let prim_x = prim.bbox.x;
        let x_tolerance = 0.02;
        if (acc_x - prim_x).abs() > x_tolerance {
            return false;
        }

        let acc_w = acc.bbox.width;
        let prim_w = prim.bbox.width;
        let w_tolerance = acc_w.max(prim_w) * 0.10;
        if (acc_w - prim_w).abs() > w_tolerance {
            return false;
        }

        match (acc.last_font_size, Self::font_size_of(prim)) {
            (Some(acc_fs), Some(prim_fs)) => (acc_fs - prim_fs).abs() < 0.1,
            _ => false,
        }
    }

    pub(super) fn is_continuation_text(text: &str) -> bool {
        let first = match text.chars().next() {
            Some(c) => c,
            None => return false,
        };
        first.is_lowercase()
            || matches!(
                first,
                ',' | ';'
                    | '.'
                    | '!'
                    | '?'
                    | ':'
                    | ')'
                    | ']'
                    | '-'
                    | '\u{2013}'
                    | '\u{2014}'
                    | '\u{2026}'
            )
    }

    pub(super) fn detect_and_mark_repeated_header(
        &mut self,
        next_page_prims: &[Primitive],
        next_page: u32,
    ) {
        let table = match &mut self.open_table {
            Some(t) => t,
            None => return,
        };

        let header_row = match table.cells.get(&0) {
            Some(r) => r,
            None => return,
        };

        let header_texts: Vec<&str> = header_row.values().map(|c| c.text.as_str()).collect();
        if header_texts.is_empty() {
            return;
        }

        let mut new_row_texts: Vec<String> = Vec::new();
        let mut first_row: Option<u32> = None;
        for prim in next_page_prims {
            let role = Self::classify(prim);
            match role {
                PrimaryRole::PageHeader { .. }
                | PrimaryRole::PageFooter { .. }
                | PrimaryRole::DecorationBare => continue,
                PrimaryRole::TableCell { row, .. }
                | PrimaryRole::TableCellContinuation { row, .. } => {
                    match first_row {
                        None => first_row = Some(row),
                        Some(fr) if row != fr => break,
                        _ => {}
                    }
                    new_row_texts.push(Self::text_of(prim));
                }
                _ => break,
            }
        }

        if new_row_texts.len() == header_texts.len()
            && new_row_texts
                .iter()
                .zip(header_texts.iter())
                .all(|(a, b)| a.trim() == b.trim())
        {
            table.repeated_header_pages.push(next_page);
        }
    }

    pub fn open_table_column_positions(&self) -> Option<Vec<f32>> {
        let table = self.open_table.as_ref()?;
        let mut xs: Vec<f32> = Vec::new();
        for row_cells in table.cells.values() {
            for cell in row_cells.values() {
                let x = cell.bbox.x;
                if !xs.iter().any(|&existing| (existing - x).abs() < 0.015) {
                    xs.push(x);
                }
            }
        }
        xs.sort_by(|a, b| a.total_cmp(b));
        if xs.len() >= 2 { Some(xs) } else { None }
    }
}
