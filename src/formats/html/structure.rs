use scraper::ElementRef;

use crate::error::Warning;

use super::render::{
    for_each_style_declaration, report_heuristic_inference, report_render_limitation,
};
use super::types::WalkState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TableCaptionPlacement {
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum StructureRole {
    List { ordered: bool },
    ListItem,
    Table,
    RowGroup { is_header: bool },
    TableCaption,
    Row,
    Cell { is_header: bool },
}

pub(crate) fn structural_role(element: ElementRef) -> Option<StructureRole> {
    aria_structure_role(element).or_else(|| display_structure_role(element))
}

pub(crate) fn is_list_container_element(element: ElementRef) -> bool {
    matches!(element.value().name(), "ul" | "ol")
        || matches!(structural_role(element), Some(StructureRole::List { .. }))
}

pub(crate) fn report_structure_inference_if_needed(
    element: ElementRef,
    state: &mut WalkState,
    warnings: &mut Vec<Warning>,
) {
    if aria_structure_role(element).is_some() {
        return;
    }

    let Some(display_value) = structural_display_value(element) else {
        return;
    };
    let tag = element.value().name();
    report_heuristic_inference(
        state,
        warnings,
        format!("css-structure:{tag}:{display_value}"),
        format!(
            "HTML structure for <{}> was inferred from CSS display:{} rather than native semantic markup or ARIA roles",
            tag, display_value
        ),
    );
}

pub(crate) fn is_table_caption_element(element: ElementRef) -> bool {
    element.value().name() == "caption"
        || matches!(structural_role(element), Some(StructureRole::TableCaption))
}

pub(crate) fn table_caption_placement(
    element: ElementRef,
    state: &mut WalkState,
    warnings: &mut Vec<Warning>,
) -> TableCaptionPlacement {
    let mut placement = TableCaptionPlacement::Top;
    for_each_style_declaration(element.value().attr("style"), |property, value| {
        if property == "caption-side" {
            placement = match value {
                "bottom" => TableCaptionPlacement::Bottom,
                "top" => TableCaptionPlacement::Top,
                other => {
                    report_render_limitation(
                        state,
                        warnings,
                        format!("caption-side:{other}"),
                        format!(
                            "HTML CSS caption-side:{} is outside the supported rendered subset and was ignored",
                            other
                        ),
                    );
                    TableCaptionPlacement::Top
                }
            };
        }
    });
    placement
}

pub(crate) fn structural_display_value(element: ElementRef) -> Option<String> {
    let mut display = None;
    for_each_style_declaration(element.value().attr("style"), |property, value| {
        if property == "display" {
            display = Some(value.to_string());
        }
    });
    match display.as_deref() {
        Some("list-item")
        | Some("table")
        | Some("inline-table")
        | Some("table-header-group")
        | Some("table-row-group")
        | Some("table-footer-group")
        | Some("table-caption")
        | Some("table-row")
        | Some("table-cell") => display,
        _ => None,
    }
}

fn aria_structure_role(element: ElementRef) -> Option<StructureRole> {
    let role = element.value().attr("role")?.trim().to_ascii_lowercase();
    match role.as_str() {
        "list" => Some(StructureRole::List { ordered: false }),
        "listitem" => Some(StructureRole::ListItem),
        "table" | "grid" | "treegrid" => Some(StructureRole::Table),
        "rowgroup" => Some(StructureRole::RowGroup { is_header: false }),
        "row" => Some(StructureRole::Row),
        "columnheader" => Some(StructureRole::Cell { is_header: true }),
        "cell" | "gridcell" | "rowheader" => Some(StructureRole::Cell { is_header: false }),
        _ => None,
    }
}

fn display_structure_role(element: ElementRef) -> Option<StructureRole> {
    match structural_display_value(element).as_deref() {
        Some("list-item") => Some(StructureRole::ListItem),
        Some("table") | Some("inline-table") => Some(StructureRole::Table),
        Some("table-header-group") => Some(StructureRole::RowGroup { is_header: true }),
        Some("table-row-group") | Some("table-footer-group") => {
            Some(StructureRole::RowGroup { is_header: false })
        }
        Some("table-caption") => Some(StructureRole::TableCaption),
        Some("table-row") => Some(StructureRole::Row),
        Some("table-cell") => Some(StructureRole::Cell { is_header: false }),
        _ => None,
    }
}
