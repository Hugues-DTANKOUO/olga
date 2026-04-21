use scraper::ElementRef;

use crate::error::Warning;
use crate::model::*;
use crate::pipeline::PrimitiveSink;

use super::super::render::report_heuristic_inference;
use super::{
    GENERIC_TEXT_CONTAINER_ELEMENTS, HtmlRenderContext, WalkState, resolve_text_direction,
};

#[derive(Debug, Clone, Copy)]
pub(crate) enum FallbackTextSemantics {
    Paragraph,
    Heading(u8),
    BlockQuote,
    CodeBlock,
    Sidebar,
}

pub(crate) fn emit_fallback_text_block(
    text: &str,
    context: &HtmlRenderContext,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
    semantics: FallbackTextSemantics,
) {
    let (text_direction, text_direction_origin) = resolve_text_direction(context, text);
    let y = state.advance_y();
    let order = state.next_order();

    let (font_size, font_name, is_bold, is_italic, bbox, hint) = match semantics {
        FallbackTextSemantics::Paragraph => (
            11.0,
            String::new(),
            false,
            false,
            BoundingBox::new(0.0, y, 1.0, state.y_step),
            SemanticHint::from_format(HintKind::Paragraph),
        ),
        FallbackTextSemantics::Heading(level) => (
            heading_font_size(level),
            String::new(),
            true,
            false,
            BoundingBox::new(0.0, y, 1.0, state.y_step),
            SemanticHint::from_format(HintKind::Heading { level }),
        ),
        FallbackTextSemantics::BlockQuote => (
            11.0,
            String::new(),
            false,
            true,
            BoundingBox::new(0.05, y, 0.9, state.y_step),
            SemanticHint::from_format(HintKind::BlockQuote),
        ),
        FallbackTextSemantics::CodeBlock => (
            10.0,
            "monospace".to_string(),
            false,
            false,
            BoundingBox::new(0.0, y, 1.0, state.y_step),
            SemanticHint::from_format(HintKind::CodeBlock),
        ),
        FallbackTextSemantics::Sidebar => (
            11.0,
            String::new(),
            false,
            false,
            BoundingBox::new(0.03, y, 0.94, state.y_step),
            SemanticHint::from_format(HintKind::Sidebar),
        ),
    };

    let primitive = Primitive::synthetic(
        PrimitiveKind::Text {
            content: text.to_string(),
            font_size,
            font_name,
            is_bold,
            is_italic,
            color: None,
            char_positions: None,
            text_direction,
            text_direction_origin,
            lang: context.lang.clone(),
            lang_origin: context.lang_origin,
        },
        bbox,
        0,
        order,
        GeometrySpace::DomFlow,
    )
    .with_hint(hint);

    primitives.emit(primitive);
}

pub(crate) fn heading_font_size(level: u8) -> f32 {
    match level {
        1 => 24.0,
        2 => 20.0,
        3 => 16.0,
        4 => 14.0,
        5 => 12.0,
        _ => 11.0,
    }
}

pub(crate) fn should_emit_generic_text_container(element: ElementRef) -> bool {
    GENERIC_TEXT_CONTAINER_ELEMENTS.contains(&element.value().name())
        || semantic_role_from_attributes(element).is_some()
}

pub(crate) fn fallback_text_semantics(element: ElementRef) -> FallbackTextSemantics {
    semantic_role_from_attributes(element).unwrap_or_else(|| match element.value().name() {
        "aside" => FallbackTextSemantics::Sidebar,
        _ => FallbackTextSemantics::Paragraph,
    })
}

pub(crate) fn report_generic_container_fallback_if_needed(
    element: ElementRef,
    semantics: FallbackTextSemantics,
    state: &mut WalkState,
    warnings: &mut Vec<Warning>,
) {
    if semantic_role_from_attributes(element).is_some() {
        return;
    }

    let tag = element.value().name();
    if !matches!(tag, "div" | "main" | "header" | "footer" | "nav" | "form") {
        return;
    }

    let semantic_name = match semantics {
        FallbackTextSemantics::Paragraph => "paragraph",
        FallbackTextSemantics::Heading(_) => "heading",
        FallbackTextSemantics::BlockQuote => "blockquote",
        FallbackTextSemantics::CodeBlock => "code block",
        FallbackTextSemantics::Sidebar => "sidebar",
    };

    report_heuristic_inference(
        state,
        warnings,
        format!("generic-container:{tag}:{semantic_name}"),
        format!(
            "HTML <{}> was emitted as a {} via generic container fallback; text semantics were inferred from container content rather than native semantic markup",
            tag, semantic_name
        ),
    );
}

fn semantic_role_from_attributes(element: ElementRef) -> Option<FallbackTextSemantics> {
    let role = element.value().attr("role")?.trim().to_ascii_lowercase();
    match role.as_str() {
        "heading" => Some(FallbackTextSemantics::Heading(
            element
                .value()
                .attr("aria-level")
                .and_then(|level| level.parse::<u8>().ok())
                .unwrap_or(1)
                .clamp(1, 6),
        )),
        "paragraph" => Some(FallbackTextSemantics::Paragraph),
        "blockquote" => Some(FallbackTextSemantics::BlockQuote),
        "code" => Some(FallbackTextSemantics::CodeBlock),
        "note" | "doc-note" | "complementary" => Some(FallbackTextSemantics::Sidebar),
        _ => None,
    }
}
