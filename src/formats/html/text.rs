#[path = "text/collect.rs"]
mod collect;
#[path = "text/fallbacks.rs"]
mod fallbacks;

use super::types::{HtmlRenderContext, HtmlWhitespaceMode, WalkState};

pub(crate) use collect::{
    collect_immediate_inline_text_content, collect_text_content, resolve_text_direction,
};
pub(crate) use fallbacks::{
    FallbackTextSemantics, emit_fallback_text_block, fallback_text_semantics, heading_font_size,
    report_generic_container_fallback_if_needed, should_emit_generic_text_container,
};

pub(crate) const SKIP_ELEMENTS: &[&str] = &[
    "script", "style", "noscript", "svg", "template", "iframe", "object", "embed", "applet",
    "link", "meta", "head",
];

pub(super) const WALK_HANDLED_ELEMENTS: &[&str] = &["ul", "ol", "dl", "table", "details"];
pub(super) const GENERIC_TEXT_CONTAINER_ELEMENTS: &[&str] = &[
    "html", "body", "div", "main", "header", "footer", "aside", "nav", "form",
];
pub(super) const SELF_EMITTING_ELEMENTS: &[&str] = &[
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "p",
    "blockquote",
    "pre",
    "li",
    "dt",
    "dd",
    "td",
    "th",
    "a",
    "article",
    "section",
    "img",
    "summary",
];
