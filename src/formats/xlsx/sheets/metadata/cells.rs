use crate::formats::xlsx::types::MergedRange;

pub(in super::super) fn parse_cell_ref(cell: &str) -> Option<(u32, u32)> {
    let mut col: u32 = 0;
    let mut row_str = String::new();

    for ch in cell.chars() {
        if ch.is_ascii_alphabetic() {
            col = col * 26 + (ch.to_ascii_uppercase() as u32 - 'A' as u32 + 1);
        } else if ch.is_ascii_digit() {
            row_str.push(ch);
        }
    }

    let row: u32 = row_str.parse().ok()?;
    if col == 0 || row == 0 {
        return None;
    }
    Some((row - 1, col - 1))
}

pub(in super::super) fn parse_cell_range(range: &str) -> Option<MergedRange> {
    let (start_row, start_col, end_row, end_col) = parse_cell_rect_ref(range)?;
    Some(MergedRange {
        start_row,
        start_col,
        rowspan: end_row - start_row + 1,
        colspan: end_col - start_col + 1,
    })
}

pub(in super::super) fn parse_cell_rect_ref(range: &str) -> Option<(u32, u32, u32, u32)> {
    let parts: Vec<&str> = range.split(':').collect();
    match parts.as_slice() {
        [single] => {
            let (row, col) = parse_cell_ref(single)?;
            Some((row, col, row, col))
        }
        [start, end] => {
            let (start_row, start_col) = parse_cell_ref(start)?;
            let (end_row, end_col) = parse_cell_ref(end)?;
            Some((start_row, start_col, end_row, end_col))
        }
        _ => None,
    }
}
