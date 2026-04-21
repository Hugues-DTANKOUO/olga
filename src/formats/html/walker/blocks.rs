use scraper::ElementRef;

use crate::model::*;
use crate::pipeline::PrimitiveSink;

use super::super::text::{collect_text_content, heading_font_size, resolve_text_direction};
use super::super::types::{HtmlRenderContext, WalkState};

struct TextBlockSpec {
    font_size: f32,
    font_name: String,
    is_bold: bool,
    is_italic: bool,
    bbox: BoundingBox,
    hint: SemanticHint,
}

fn emit_text_with_hint(
    text: String,
    context: &HtmlRenderContext,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
    spec: TextBlockSpec,
) {
    let (text_direction, text_direction_origin) = resolve_text_direction(context, &text);
    let order = state.next_order();
    let primitive = Primitive::synthetic(
        PrimitiveKind::Text {
            content: text,
            font_size: spec.font_size,
            font_name: spec.font_name,
            is_bold: spec.is_bold,
            is_italic: spec.is_italic,
            color: None,
            char_positions: None,
            text_direction,
            text_direction_origin,
            lang: context.lang.clone(),
            lang_origin: context.lang_origin,
        },
        spec.bbox,
        0,
        order,
        GeometrySpace::DomFlow,
    )
    .with_hint(spec.hint);
    primitives.emit(primitive);
}

pub(super) fn emit_heading(
    element: ElementRef,
    context: &HtmlRenderContext,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
    level: u8,
) {
    let text = collect_text_content(element, context);
    if text.is_empty() {
        return;
    }

    let y = state.advance_y();
    emit_text_with_hint(
        text,
        context,
        state,
        primitives,
        TextBlockSpec {
            font_size: heading_font_size(level),
            font_name: String::new(),
            is_bold: true,
            is_italic: false,
            bbox: BoundingBox::new(0.0, y, 1.0, state.y_step),
            hint: SemanticHint::from_format(HintKind::Heading { level }),
        },
    );
}

pub(super) fn emit_paragraph(
    element: ElementRef,
    context: &HtmlRenderContext,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
) {
    let text = collect_text_content(element, context);
    if text.is_empty() {
        return;
    }

    let y = state.advance_y();
    emit_text_with_hint(
        text,
        context,
        state,
        primitives,
        TextBlockSpec {
            font_size: 11.0,
            font_name: String::new(),
            is_bold: false,
            is_italic: false,
            bbox: BoundingBox::new(0.0, y, 1.0, state.y_step),
            hint: SemanticHint::from_format(HintKind::Paragraph),
        },
    );
}

/// Emit a `<summary>` as a bold paragraph block.
///
/// `<summary>` is the always-visible label for its parent `<details>`
/// disclosure widget — the "question" in an FAQ, the "title" of a
/// collapsible section. It is rendered regardless of whether `<details>`
/// is open or closed, which is why every document extractor that preserves
/// author intent (pandoc, tika, readability, html2text) surfaces its text.
/// Dropping it silently hides content the author explicitly labelled as
/// user-visible.
///
/// Emitted as a bold paragraph rather than a heading to avoid inventing
/// heading levels that don't exist in the authored structure. Downstream
/// consumers that want heading semantics can use `role="heading"` inside
/// the summary; the walker honours that via the existing semantic-role
/// fallback path.
pub(super) fn emit_summary(
    element: ElementRef,
    context: &HtmlRenderContext,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
) {
    let text = collect_text_content(element, context);
    if text.is_empty() {
        return;
    }

    let y = state.advance_y();
    emit_text_with_hint(
        text,
        context,
        state,
        primitives,
        TextBlockSpec {
            font_size: 11.0,
            font_name: String::new(),
            is_bold: true,
            is_italic: false,
            bbox: BoundingBox::new(0.0, y, 1.0, state.y_step),
            hint: SemanticHint::from_format(HintKind::Paragraph),
        },
    );
}

pub(super) fn emit_blockquote(
    element: ElementRef,
    context: &HtmlRenderContext,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
) {
    let text = collect_text_content(element, context);
    if text.is_empty() {
        return;
    }

    let y = state.advance_y();
    emit_text_with_hint(
        text,
        context,
        state,
        primitives,
        TextBlockSpec {
            font_size: 11.0,
            font_name: String::new(),
            is_bold: false,
            is_italic: true,
            bbox: BoundingBox::new(0.05, y, 0.9, state.y_step),
            hint: SemanticHint::from_format(HintKind::BlockQuote),
        },
    );
}

pub(super) fn emit_code_block(
    element: ElementRef,
    context: &HtmlRenderContext,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
) {
    let text = collect_text_content(element, context);
    if text.is_empty() {
        return;
    }

    let y = state.advance_y();
    emit_text_with_hint(
        text,
        context,
        state,
        primitives,
        TextBlockSpec {
            font_size: 10.0,
            font_name: "monospace".to_string(),
            is_bold: false,
            is_italic: false,
            bbox: BoundingBox::new(0.0, y, 1.0, state.y_step),
            hint: SemanticHint::from_format(HintKind::CodeBlock),
        },
    );
}

pub(super) fn emit_definition_term(
    element: ElementRef,
    context: &HtmlRenderContext,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
) {
    let text = collect_text_content(element, context);
    if text.is_empty() {
        return;
    }

    let y = state.advance_y();
    emit_text_with_hint(
        text,
        context,
        state,
        primitives,
        TextBlockSpec {
            font_size: 11.0,
            font_name: String::new(),
            is_bold: true,
            is_italic: false,
            bbox: BoundingBox::new(0.0, y, 1.0, state.y_step),
            hint: SemanticHint::from_format(HintKind::ListItem {
                depth: 0,
                ordered: false,
                list_group: None,
            }),
        },
    );
}

pub(super) fn emit_definition_detail(
    element: ElementRef,
    context: &HtmlRenderContext,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
) {
    let text = collect_text_content(element, context);
    if text.is_empty() {
        return;
    }

    let y = state.advance_y();
    emit_text_with_hint(
        text,
        context,
        state,
        primitives,
        TextBlockSpec {
            font_size: 11.0,
            font_name: String::new(),
            is_bold: false,
            is_italic: false,
            bbox: BoundingBox::new(0.03, y, 0.97, state.y_step),
            hint: SemanticHint::from_format(HintKind::ListItem {
                depth: 1,
                ordered: false,
                list_group: None,
            }),
        },
    );
}

pub(super) fn emit_image(
    element: ElementRef,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
) {
    let alt = element.value().attr("alt").map(|s| s.to_string());
    let y = state.advance_y();
    let order = state.next_order();
    let primitive = Primitive::synthetic(
        PrimitiveKind::Image {
            format: ImageFormat::Unknown,
            data: Vec::new(),
            alt_text: alt,
        },
        BoundingBox::new(0.0, y, 1.0, state.y_step),
        0,
        order,
        GeometrySpace::DomFlow,
    );
    primitives.emit(primitive);
}

pub(super) fn emit_link(
    element: ElementRef,
    context: &HtmlRenderContext,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
) {
    let text = collect_text_content(element, context);
    if text.is_empty() {
        return;
    }

    let href = element.value().attr("href").map(|s| s.to_string());
    let y = state.advance_y();
    let (text_direction, text_direction_origin) = resolve_text_direction(context, &text);
    let order = state.next_order();
    let mut primitive = Primitive::synthetic(
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
        BoundingBox::new(0.0, y, 1.0, state.y_step),
        0,
        order,
        GeometrySpace::DomFlow,
    );
    if let Some(url) = href {
        primitive = primitive.with_hint(SemanticHint::from_format(HintKind::Link { url }));
    }
    primitives.emit(primitive);
}

pub(super) fn emit_section_start(
    element: ElementRef,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
) {
    let title = element.value().attr("aria-label").map(|s| s.to_string());
    let y = state.advance_y();
    let order = state.next_order();
    let primitive = Primitive::synthetic(
        PrimitiveKind::Text {
            content: String::new(),
            font_size: 0.0,
            font_name: String::new(),
            is_bold: false,
            is_italic: false,
            color: None,
            char_positions: None,
            text_direction: TextDirection::default(),
            text_direction_origin: ValueOrigin::Synthetic,
            lang: None,
            lang_origin: None,
        },
        BoundingBox::new(0.0, y, 1.0, 0.0),
        0,
        order,
        GeometrySpace::DomFlow,
    )
    .with_hint(SemanticHint::from_format(HintKind::SectionStart { title }));
    primitives.emit(primitive);
}

pub(super) fn emit_section_end(state: &mut WalkState, primitives: &mut impl PrimitiveSink) {
    let order = state.next_order();
    let primitive = Primitive::synthetic(
        PrimitiveKind::Text {
            content: String::new(),
            font_size: 0.0,
            font_name: String::new(),
            is_bold: false,
            is_italic: false,
            color: None,
            char_positions: None,
            text_direction: TextDirection::default(),
            text_direction_origin: ValueOrigin::Synthetic,
            lang: None,
            lang_origin: None,
        },
        BoundingBox::new(0.0, state.y_position, 1.0, 0.0),
        0,
        order,
        GeometrySpace::DomFlow,
    )
    .with_hint(SemanticHint::from_format(HintKind::SectionEnd));
    primitives.emit(primitive);
}
