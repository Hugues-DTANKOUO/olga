use quick_xml::events::Event;
use quick_xml::reader::Reader as XmlReader;

use crate::error::{Warning, WarningKind};
use crate::formats::xml_utils::{attr_value, local_name};

use super::super::cells::parse_cell_rect_ref;
use crate::formats::xlsx::types::NativeTable;

pub(super) fn parse_table_xml(
    xml: &[u8],
    sheet_name: &str,
    page: u32,
    warnings: &mut Vec<Warning>,
) -> Option<NativeTable> {
    let mut reader = XmlReader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e))
                if local_name(e.name().as_ref()) == b"table" =>
            {
                let mut table_ref = None;
                let mut header_row_count = 1;
                for attr in e.attributes().flatten() {
                    match local_name(attr.key.as_ref()).as_slice() {
                        b"ref" => table_ref = Some(attr_value(&attr)),
                        b"headerRowCount" => {
                            header_row_count =
                                attr_value(&attr).parse::<u32>().unwrap_or(header_row_count);
                        }
                        _ => {}
                    }
                }

                let Some(table_ref) = table_ref else {
                    warnings.push(Warning {
                        kind: WarningKind::MalformedContent,
                        message: format!(
                            "SpreadsheetML table in sheet '{}' is missing a ref attribute",
                            sheet_name
                        ),
                        page: Some(page),
                    });
                    return None;
                };

                return match parse_cell_rect_ref(&table_ref) {
                    Some((start_row, start_col, end_row, end_col)) => Some(NativeTable {
                        start_row,
                        start_col,
                        end_row,
                        end_col,
                        header_row_count,
                    }),
                    None => {
                        warnings.push(Warning {
                            kind: WarningKind::MalformedContent,
                            message: format!(
                                "SpreadsheetML table ref '{}' in sheet '{}' is malformed",
                                table_ref, sheet_name
                            ),
                            page: Some(page),
                        });
                        None
                    }
                };
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                warnings.push(Warning {
                    kind: WarningKind::MalformedContent,
                    message: format!(
                        "Failed to parse table metadata for sheet '{}': {}",
                        sheet_name, e
                    ),
                    page: Some(page),
                });
                return None;
            }
            _ => {}
        }
        buf.clear();
    }

    warnings.push(Warning {
        kind: WarningKind::MalformedContent,
        message: format!(
            "SpreadsheetML table metadata for sheet '{}' did not contain a table root element",
            sheet_name
        ),
        page: Some(page),
    });
    None
}
