use super::*;

impl TableDetector {
    /// Detect whether the first row of a table is a header row.
    ///
    /// **Must be called with the original center-based assignments** (before
    /// span inference adjusts row indices), so that text whose bbox extends
    /// upward from row 1 into row 0 is not misclassified as row-0 content.
    ///
    /// A row is classified as a header if it differs from body rows in at
    /// least one of these signals:
    ///
    /// 1. **Bold text**: all text in row 0 is bold, but not all body text is.
    /// 2. **Font size**: row 0 has a larger average font size than body rows.
    /// 3. **Different font**: row 0 uses a different font name than the
    ///    majority of body rows.
    ///
    /// Returns `true` if the first row should be emitted as `TableHeader`
    /// instead of `TableCell`.
    pub(super) fn detect_header_row(
        &self,
        assignments: &[(usize, usize, usize)],
        page: &[Primitive],
        table: &cells::Table,
    ) -> bool {
        if table.num_rows() < 2 {
            return false;
        }

        let mut row0_bold_count = 0usize;
        let mut row0_total = 0usize;
        let mut row0_font_sizes: Vec<f32> = Vec::new();
        let mut row0_fonts: Vec<String> = Vec::new();

        let mut body_bold_count = 0usize;
        let mut body_total = 0usize;
        let mut body_font_sizes: Vec<f32> = Vec::new();
        let mut body_fonts: Vec<String> = Vec::new();

        for &(idx, row, _) in assignments {
            if let PrimitiveKind::Text {
                is_bold,
                font_size,
                ref font_name,
                ..
            } = page[idx].kind
            {
                if row == 0 {
                    row0_total += 1;
                    if is_bold {
                        row0_bold_count += 1;
                    }
                    row0_font_sizes.push(font_size);
                    row0_fonts.push(font_name.clone());
                } else {
                    body_total += 1;
                    if is_bold {
                        body_bold_count += 1;
                    }
                    body_font_sizes.push(font_size);
                    body_fonts.push(font_name.clone());
                }
            }
        }

        if row0_total == 0 || body_total == 0 {
            return false;
        }

        let row0_all_bold = row0_bold_count == row0_total;
        let body_all_bold = body_bold_count == body_total;
        if row0_all_bold && !body_all_bold {
            return true;
        }

        let row0_avg_size = row0_font_sizes.iter().sum::<f32>() / row0_total as f32;
        let body_avg_size = body_font_sizes.iter().sum::<f32>() / body_total as f32;
        if body_avg_size > 0.0 && row0_avg_size > body_avg_size * 1.1 {
            return true;
        }

        if !row0_fonts.is_empty() && !body_fonts.is_empty() {
            let row0_dominant = most_common(&row0_fonts);
            let body_dominant = most_common(&body_fonts);
            if row0_dominant != body_dominant {
                return true;
            }
        }

        false
    }
}

fn most_common(items: &[String]) -> &str {
    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for item in items {
        *counts.entry(item.as_str()).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by_key(|&(_, count)| count)
        .map(|(s, _)| s)
        .unwrap_or("")
}
