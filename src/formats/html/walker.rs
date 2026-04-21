//! DOM tree walker — converts HTML elements to Primitives with SemanticHints.
//!
//! The walker does a depth-first traversal of the parsed DOM, skipping
//! non-content elements and applying a small rendered-subset contract:
//! visibility, directionality, and whitespace are respected from HTML
//! attributes and inline styles, while geometry remains synthetic DOM flow.
//!
//! Positions emitted by this decoder are synthetic, normalized coordinates that
//! preserve DOM order and nesting hints. They are not physical layout boxes.
//!
//! ## Coverage policy
//!
//! Mainstream extractors (pandoc, Apache Tika, Readability, html2text) use an
//! *inclusive by default* strategy: parse everything, try to classify, emit
//! whatever looks like content, stay tolerant of unknown markup. Our default
//! is more precision-oriented: known block elements have explicit handlers
//! that carry structural `SemanticHint`s (Heading, ListItem, TableCell,
//! Paragraph with proper styling) so downstream structure detectors don't
//! have to re-infer the author's intent.
//!
//! The cost of that precision is recall: HTML5 adds elements over time
//! (`<details>`, `<summary>`, `<dialog>`, `<figcaption>`, custom web
//! components) and every gap in the allowlist is content we drop while
//! pandoc/Tika surface it. Where we hit a concrete recall gap, we add the
//! explicit handler (preserving the semantic hint) rather than a generic
//! catch-all, because the explicit path preserves our structural advantage.
//! The generic-container fallback (see `text::should_emit_generic_text_container`)
//! catches loose text in `<div>`/`<main>`/`<header>` etc., but it does
//! *not* cover block-level unknown elements — those must get explicit
//! handlers if they carry content.
//!
//! Visibility is derived per the HTML/CSS specs, not per the ARIA spec —
//! see `is_hidden_element` in `render.rs` for the precise list of signals
//! we respect (and why `aria-hidden` is not one of them).

mod blocks;
mod forms;
mod structures;

use scraper::ElementRef;

use crate::error::{Warning, WarningKind};
use crate::pipeline::PrimitiveSink;

use self::blocks::{
    emit_blockquote, emit_code_block, emit_definition_detail, emit_definition_term, emit_heading,
    emit_image, emit_link, emit_paragraph, emit_section_end, emit_section_start, emit_summary,
};
use self::forms::{
    emit_button, emit_form_control, emit_label, emit_option, walk_form, walk_select,
};
use self::structures::{
    handle_structure_role, walk_list_container, walk_list_item, walk_table_caption,
    walk_table_cell, walk_table_container, walk_table_row, walk_table_row_group,
};
use super::render::derive_render_context;
use super::structure::{report_structure_inference_if_needed, structural_role};
use super::text::{
    SKIP_ELEMENTS, collect_immediate_inline_text_content, emit_fallback_text_block,
    fallback_text_semantics, report_generic_container_fallback_if_needed,
    should_emit_generic_text_container,
};
use super::types::{HtmlRenderContext, WalkState};

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Walk the parsed HTML DOM and emit Primitives.
pub(crate) fn walk_dom(
    document: &scraper::Html,
    primitives: &mut impl PrimitiveSink,
    warnings: &mut Vec<Warning>,
) {
    let mut state = WalkState::new();
    let root = document.root_element();
    let root_context = HtmlRenderContext::default();
    walk_node(root, &root_context, &mut state, primitives, warnings);
}

// ---------------------------------------------------------------------------
// Recursive walker
// ---------------------------------------------------------------------------

/// Maximum DOM depth to prevent stack overflow on deeply nested HTML.
const MAX_DOM_DEPTH: u32 = 128;

fn walk_node(
    element: ElementRef,
    parent_context: &HtmlRenderContext,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
    warnings: &mut Vec<Warning>,
) {
    if state.dom_depth >= MAX_DOM_DEPTH {
        if !state.depth_limit_hit {
            warnings.push(Warning {
                kind: WarningKind::TruncatedContent,
                message: format!(
                    "HTML DOM depth exceeded {} levels; deeper content was truncated",
                    MAX_DOM_DEPTH
                ),
                page: Some(0),
            });
            state.depth_limit_hit = true;
        }
        return;
    }
    state.dom_depth += 1;

    let tag = element.value().name();

    // Skip non-content elements entirely.
    if SKIP_ELEMENTS.contains(&tag) {
        state.dom_depth -= 1;
        return;
    }

    let Some(context) = derive_render_context(element, parent_context, state, warnings) else {
        state.dom_depth -= 1;
        return;
    };

    if let Some(role) = structural_role(element) {
        report_structure_inference_if_needed(element, state, warnings);
        handle_structure_role(role, element, &context, state, primitives, warnings);
        state.dom_depth -= 1;
        return;
    }

    match tag {
        // -- Headings --
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let level = tag
                .as_bytes()
                .get(1)
                .map(|b| b - b'0')
                .unwrap_or(1)
                .clamp(1, 6);
            emit_heading(element, &context, state, primitives, level);
        }

        // -- Paragraphs --
        "p" => {
            emit_paragraph(element, &context, state, primitives);
        }

        // -- Blockquote --
        "blockquote" => {
            emit_blockquote(element, &context, state, primitives);
        }

        // -- Code blocks --
        "pre" => {
            emit_code_block(element, &context, state, primitives);
        }

        // -- Lists --
        "ul" | "ol" => {
            walk_list_container(element, &context, state, primitives, warnings, tag == "ol");
        }

        "li" => {
            walk_list_item(element, &context, state, primitives, warnings);
        }

        // -- Definition lists --
        "dl" => {
            walk_children(element, &context, state, primitives, warnings);
        }

        "dt" => {
            emit_definition_term(element, &context, state, primitives);
        }

        "dd" => {
            emit_definition_detail(element, &context, state, primitives);
        }

        // -- Tables --
        "table" => {
            walk_table_container(element, &context, state, primitives, warnings);
        }

        "thead" | "tbody" | "tfoot" => {
            walk_table_row_group(
                element,
                &context,
                state,
                primitives,
                warnings,
                tag == "thead",
            );
        }

        "caption" => {
            walk_table_caption(element, &context, state, primitives);
        }

        "tr" => {
            walk_table_row(element, &context, state, primitives, warnings);
        }

        "td" | "th" => {
            walk_table_cell(element, &context, state, primitives, warnings, tag == "th");
        }

        // -- Images --
        "img" => {
            emit_image(element, state, primitives);
        }

        // -- Horizontal rule (page break hint) --
        "hr" => {
            state.advance_y();
        }

        // -- Links --
        "a" => {
            emit_link(element, &context, state, primitives);
        }

        // -- Semantic sections --
        "article" | "section" => {
            emit_section_start(element, state, primitives);
            walk_children(element, &context, state, primitives, warnings);
            emit_section_end(state, primitives);
        }

        // -- Disclosure widget (HTML Living Standard § 4.11.1) --
        //
        // `<details>` itself emits no primitive — it's a container whose
        // `<summary>` and body contribute the visible text, both of which
        // render regardless of the open/closed state. Recursing through
        // children preserves DOM order so the summary surfaces before its
        // explanatory paragraph.
        "details" => {
            walk_children(element, &context, state, primitives, warnings);
        }

        "summary" => {
            emit_summary(element, &context, state, primitives);
        }

        // -- Forms (see walker/forms.rs) --
        //
        // `<form>` itself emits nothing — its descendants carry the content.
        // This preserves DOM order and prevents the generic-container fallback
        // from concatenating labels, options, and button text into one block.
        "form" => {
            walk_form(element, &context, state, primitives, warnings);
        }

        "label" => {
            emit_label(element, &context, state, primitives);
        }

        "select" => {
            walk_select(element, &context, state, primitives, warnings);
        }

        "option" => {
            emit_option(element, &context, state, primitives);
        }

        "button" => {
            emit_button(element, &context, state, primitives);
        }

        "input" | "textarea" => {
            emit_form_control(element, &context, state, primitives);
        }

        // -- Generic containers: just recurse --
        _ => {
            if should_emit_generic_text_container(element) {
                let text = collect_immediate_inline_text_content(element, &context);
                if !text.is_empty() {
                    report_generic_container_fallback_if_needed(
                        element,
                        fallback_text_semantics(element),
                        state,
                        warnings,
                    );
                    emit_fallback_text_block(
                        &text,
                        &context,
                        state,
                        primitives,
                        fallback_text_semantics(element),
                    );
                }
            }
            walk_children(element, &context, state, primitives, warnings);
        }
    }

    state.dom_depth -= 1;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Walk all child elements of a node.
fn walk_children(
    element: ElementRef,
    context: &HtmlRenderContext,
    state: &mut WalkState,
    primitives: &mut impl PrimitiveSink,
    warnings: &mut Vec<Warning>,
) {
    for child in element.children() {
        if let Some(child_el) = ElementRef::wrap(child) {
            walk_node(child_el, context, state, primitives, warnings);
        }
    }
}

#[cfg(test)]
mod tests;
