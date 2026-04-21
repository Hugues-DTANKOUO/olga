//! HTML form element handling.
//!
//! Preserves DOM order and semantic structure of interactive forms rather
//! than collapsing their contents into a single concatenated text run.
//!
//! The generic-container fallback (see `text::should_emit_generic_text_container`)
//! flattened everything inside `<form>` into one paragraph because labels,
//! selects, options, buttons and inputs were unknown to the walker. This
//! module handles each form-related element explicitly so that:
//!
//! * `<h3>` (and any other heading) inside the form emits first, in DOM order
//! * `<label>` emits its own paragraph block (its text, not concatenated)
//! * `<select>` with `<option>` children emits as an unordered list of choices
//! * `<button>` emits its visible text as a paragraph block
//! * `<textarea>` / `<input>` contribute their placeholder or value as a
//!   paragraph block if present, otherwise nothing
//!
//! DOM order is preserved because `<form>` just walks its children without
//! pre-emitting any fallback container text.
//!
//! Authoring references:
//! * HTML Living Standard § 4.10 "Forms" — label / select / option semantics
//! * WCAG 2.1 — SC 1.3.1 Info and Relationships — labels are not presentational
//! * ARIA Authoring Practices — `listbox` / `option` pattern maps naturally

use scraper::ElementRef;

use crate::error::Warning;
use crate::model::*;
use crate::pipeline::PrimitiveSink;

use super::super::text::{collect_text_content, resolve_text_direction};
use super::super::types::{HtmlRenderContext, ListContext, WalkState};
use super::walk_children;

/// Walk a `<form>` container. Emits nothing itself — child elements (labels,
/// selects, buttons, inputs, headings) are responsible for their own blocks.
/// This preserves DOM order, so an `<h3>` placed first inside a `<form>`
/// correctly surfaces before the form's controls.
pub(super) fn walk_form(
    element: ElementRef,
    context: &HtmlRenderContext,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
    warnings: &mut Vec<Warning>,
) {
    walk_children(element, context, state, primitives, warnings);
}

/// Emit a `<label>` as a paragraph block.
///
/// Text is collected from the label's entire subtree (inline children,
/// nested `<a>`, wrapped checkboxes' sibling text). Children are NOT walked
/// recursively afterwards — that mirrors how `<p>` is handled and prevents
/// nested links or form controls from double-emitting.
pub(super) fn emit_label(
    element: ElementRef,
    context: &HtmlRenderContext,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
) {
    let text = collect_text_content(element, context);
    if text.is_empty() {
        return;
    }
    emit_paragraph_block(text, context, state, primitives);
}

/// Walk a `<select>` and emit its `<option>` children as an unordered list.
///
/// The select itself has no visible text of its own; the options are the
/// enumerated choices. Emitting them as list items preserves the "one of
/// many" semantics that downstream structure detectors and renderers can
/// recognize, matching how screen readers announce a listbox.
pub(super) fn walk_select(
    element: ElementRef,
    context: &HtmlRenderContext,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
    warnings: &mut Vec<Warning>,
) {
    let depth = state.list_stack.len() as u8;
    state.list_stack.push(ListContext {
        depth,
        ordered: false,
    });
    walk_children(element, context, state, primitives, warnings);
    state.list_stack.pop();
}

/// Emit an `<option>` as a list item within its surrounding `<select>`.
///
/// If encountered outside a `<select>` context the list stack is empty and we
/// fall back to a depth-0 unordered item, which still renders sensibly.
pub(super) fn emit_option(
    element: ElementRef,
    context: &HtmlRenderContext,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
) {
    let text = collect_text_content(element, context);
    if text.is_empty() {
        return;
    }

    let (depth, ordered) = state
        .list_stack
        .last()
        .map(|ctx| (ctx.depth, ctx.ordered))
        .unwrap_or((0, false));

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

/// Emit a `<button>` as a paragraph block carrying its visible label.
pub(super) fn emit_button(
    element: ElementRef,
    context: &HtmlRenderContext,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
) {
    let text = collect_text_content(element, context);
    if text.is_empty() {
        return;
    }
    emit_paragraph_block(text, context, state, primitives);
}

/// Emit a standalone `<input>` or `<textarea>`.
///
/// Most text inputs carry no author content; they surface a placeholder or
/// pre-filled `value`. Checkboxes and hidden inputs carry neither visible
/// text nor user-relevant placeholder, so nothing is emitted. An empty
/// `<textarea>` similarly emits nothing.
///
/// When a placeholder IS present we emit it as a paragraph — it's author-
/// authored helper text that describes what the control accepts (e.g.
/// `placeholder="e.g. KPS-P-04412"`).
pub(super) fn emit_form_control(
    element: ElementRef,
    context: &HtmlRenderContext,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
) {
    let tag = element.value().name();

    // Prefer explicit value (pre-filled input), then placeholder, then
    // textarea body text.
    let text = match tag {
        "input" => {
            let input_type = element
                .value()
                .attr("type")
                .unwrap_or("text")
                .to_ascii_lowercase();
            if matches!(
                input_type.as_str(),
                "hidden" | "checkbox" | "radio" | "submit" | "reset" | "button" | "image" | "file"
            ) {
                return;
            }
            element
                .value()
                .attr("value")
                .filter(|v| !v.is_empty())
                .or_else(|| element.value().attr("placeholder"))
                .unwrap_or("")
                .to_string()
        }
        "textarea" => collect_text_content(element, context),
        _ => return,
    };

    let text = text.trim().to_string();
    if text.is_empty() {
        return;
    }

    emit_paragraph_block(text, context, state, primitives);
}

fn emit_paragraph_block(
    text: String,
    context: &HtmlRenderContext,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
) {
    let (text_direction, text_direction_origin) = resolve_text_direction(context, &text);
    let y = state.advance_y();
    let order = state.next_order();
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
        BoundingBox::new(0.0, y, 1.0, state.y_step),
        0,
        order,
        GeometrySpace::DomFlow,
    )
    .with_hint(SemanticHint::from_format(HintKind::Paragraph));
    primitives.emit(primitive);
}
