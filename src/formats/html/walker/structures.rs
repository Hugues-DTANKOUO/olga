use scraper::ElementRef;

use crate::error::{Warning, WarningKind};
use crate::model::*;
use crate::pipeline::PrimitiveSink;

use super::super::structure::{
    StructureRole, TableCaptionPlacement, is_table_caption_element, table_caption_placement,
};
use super::super::text::{
    FallbackTextSemantics, collect_text_content, emit_fallback_text_block, resolve_text_direction,
};
use super::super::types::{HtmlRenderContext, ListContext, WalkState};
use super::{walk_children, walk_node};

pub(super) fn handle_structure_role(
    role: StructureRole,
    element: ElementRef,
    context: &HtmlRenderContext,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
    warnings: &mut Vec<Warning>,
) {
    match role {
        StructureRole::List { ordered } => {
            walk_list_container(element, context, state, primitives, warnings, ordered);
        }
        StructureRole::ListItem => {
            walk_list_item(element, context, state, primitives, warnings);
        }
        StructureRole::Table => {
            walk_table_container(element, context, state, primitives, warnings);
        }
        StructureRole::RowGroup { is_header } => {
            walk_table_row_group(element, context, state, primitives, warnings, is_header);
        }
        StructureRole::TableCaption => {
            walk_table_caption(element, context, state, primitives);
        }
        StructureRole::Row => {
            walk_table_row(element, context, state, primitives, warnings);
        }
        StructureRole::Cell { is_header } => {
            walk_table_cell(element, context, state, primitives, warnings, is_header);
        }
    }
}

pub(super) fn walk_list_container(
    element: ElementRef,
    context: &HtmlRenderContext,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
    warnings: &mut Vec<Warning>,
    ordered: bool,
) {
    let depth = state.list_stack.len() as u8;
    let effective_ordered = ordered || context.list_ordered_hint.unwrap_or(false);
    state.list_stack.push(ListContext {
        depth,
        ordered: effective_ordered,
    });
    walk_children(element, context, state, primitives, warnings);
    state.list_stack.pop();
}

pub(super) fn walk_list_item(
    element: ElementRef,
    context: &HtmlRenderContext,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
    warnings: &mut Vec<Warning>,
) {
    let list_ctx = state.list_stack.last().cloned();
    let text = collect_text_content(element, context);
    if !text.is_empty() {
        let (depth, ordered) = list_ctx
            .map(|ctx| (ctx.depth, ctx.ordered))
            .unwrap_or((0, context.list_ordered_hint.unwrap_or(false)));
        let (text_direction, text_direction_origin) = resolve_text_direction(context, &text);
        let y = state.advance_y();
        let order = state.next_order();
        let indent = 0.03 * (depth as f32 + 1.0);
        let primitive = Primitive::synthetic(
            PrimitiveKind::Text {
                content: text,
                font_size: 11.0,
                font_name: String::new(),
                is_bold: false,
                is_italic: false,
                color: None,
                char_positions: None,
                text_direction,
                text_direction_origin,
                lang: context.lang.clone(),
                lang_origin: context.lang_origin,
            },
            BoundingBox::new(indent, y, 1.0 - indent, state.y_step),
            0,
            order,
            GeometrySpace::DomFlow,
        )
        .with_hint(SemanticHint::from_format(HintKind::ListItem {
            depth,
            ordered,
            list_group: None,
        }));
        primitives.emit(primitive);
    }

    for child in element.children() {
        if let Some(child_element) = ElementRef::wrap(child)
            && super::super::structure::is_list_container_element(child_element)
        {
            walk_node(child_element, context, state, primitives, warnings);
        }
    }
}

pub(super) fn walk_table_container(
    element: ElementRef,
    context: &HtmlRenderContext,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
    warnings: &mut Vec<Warning>,
) {
    state
        .table_stack
        .push(super::super::types::TableContext::new());
    for child in element.children() {
        let Some(child_el) = ElementRef::wrap(child) else {
            continue;
        };
        if is_table_caption_element(child_el)
            && table_caption_placement(child_el, state, warnings) == TableCaptionPlacement::Top
        {
            walk_table_caption(child_el, context, state, primitives);
        }
    }
    for child in element.children() {
        let Some(child_el) = ElementRef::wrap(child) else {
            continue;
        };
        if is_table_caption_element(child_el) {
            continue;
        }
        walk_node(child_el, context, state, primitives, warnings);
    }
    for child in element.children() {
        let Some(child_el) = ElementRef::wrap(child) else {
            continue;
        };
        if is_table_caption_element(child_el)
            && table_caption_placement(child_el, state, warnings) == TableCaptionPlacement::Bottom
        {
            walk_table_caption(child_el, context, state, primitives);
        }
    }
    state.table_stack.pop();
}

pub(super) fn walk_table_row_group(
    element: ElementRef,
    context: &HtmlRenderContext,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
    warnings: &mut Vec<Warning>,
    is_header_group: bool,
) {
    let previous = state
        .table_stack
        .last()
        .map(|table| table.current_group_is_header)
        .unwrap_or(false);
    if let Some(table) = state.table_stack.last_mut() {
        table.current_group_is_header = is_header_group;
    }
    walk_children(element, context, state, primitives, warnings);
    if let Some(table) = state.table_stack.last_mut() {
        table.current_group_is_header = previous;
    }
}

pub(super) fn walk_table_row(
    element: ElementRef,
    context: &HtmlRenderContext,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
    warnings: &mut Vec<Warning>,
) {
    if let Some(table) = state.table_stack.last_mut() {
        table.col = 0;
        table.next_free_col();
    }
    walk_children(element, context, state, primitives, warnings);
    if let Some(table) = state.table_stack.last_mut() {
        table.row += 1;
    }
}

pub(super) fn walk_table_cell(
    element: ElementRef,
    context: &HtmlRenderContext,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
    warnings: &mut Vec<Warning>,
    is_header: bool,
) {
    let text = collect_text_content(element, context);

    let (row, col) = state
        .table_stack
        .last()
        .map(|table| (table.row, table.col))
        .unwrap_or((0, 0));
    let colspan = parse_table_span(
        element.value().attr("colspan"),
        "colspan",
        row,
        col,
        warnings,
    );
    let rowspan = parse_table_span(
        element.value().attr("rowspan"),
        "rowspan",
        row,
        col,
        warnings,
    );

    let effective_header = is_header
        || state
            .table_stack
            .last()
            .is_some_and(|table| table.current_group_is_header);

    if !text.is_empty() {
        let (text_direction, text_direction_origin) = resolve_text_direction(context, &text);
        let y = state.advance_y();
        let order = state.next_order();
        let hint = if effective_header {
            SemanticHint::from_format(HintKind::TableHeader { col })
        } else {
            SemanticHint::from_format(HintKind::TableCell {
                row,
                col,
                rowspan,
                colspan,
            })
        };
        let primitive = Primitive::synthetic(
            PrimitiveKind::Text {
                content: text,
                font_size: 11.0,
                font_name: String::new(),
                is_bold: effective_header,
                is_italic: false,
                color: None,
                char_positions: None,
                text_direction,
                text_direction_origin,
                lang: context.lang.clone(),
                lang_origin: context.lang_origin,
            },
            BoundingBox::new(0.0, y, 1.0, state.y_step),
            0,
            order,
            GeometrySpace::DomFlow,
        )
        .with_hint(hint);
        primitives.emit(primitive);
    }

    if let Some(table) = state.table_stack.last_mut() {
        table.mark_occupied(row, col, rowspan, colspan);
        table.col += colspan;
        table.next_free_col();
    }
}

pub(super) fn walk_table_caption(
    element: ElementRef,
    context: &HtmlRenderContext,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
) {
    let text = collect_text_content(element, context);
    if !text.is_empty() {
        emit_fallback_text_block(
            &text,
            context,
            state,
            primitives,
            FallbackTextSemantics::Paragraph,
        );
    }
}

fn parse_table_span(
    raw_value: Option<&str>,
    attr_name: &str,
    row: u32,
    col: u32,
    warnings: &mut Vec<Warning>,
) -> u32 {
    const MAX_SPAN: u32 = 256;

    let parsed = raw_value
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(1);
    let clamped = parsed.clamp(1, MAX_SPAN);
    if parsed != clamped {
        warnings.push(Warning {
            kind: WarningKind::TruncatedContent,
            message: format!(
                "HTML table {}={} at row {}, col {} was clamped to {}",
                attr_name, parsed, row, col, clamped
            ),
            page: Some(0),
        });
    }
    clamped
}
