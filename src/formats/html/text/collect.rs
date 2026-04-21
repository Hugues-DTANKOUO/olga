use ego_tree::NodeRef;
use pdf_oxide::text::rtl_detector::is_rtl_text;
use scraper::{ElementRef, Node};
use unicode_normalization::UnicodeNormalization;

use crate::model::{TextDirection, ValueOrigin};

use super::super::render::{derive_whitespace_mode, is_hidden_element};
use super::super::structure::structural_role;
use super::{
    GENERIC_TEXT_CONTAINER_ELEMENTS, HtmlRenderContext, HtmlWhitespaceMode, SELF_EMITTING_ELEMENTS,
    SKIP_ELEMENTS, WALK_HANDLED_ELEMENTS,
};

#[derive(Debug, Default)]
struct TextCollector {
    buffer: String,
    last_was_space: bool,
    last_was_line_break: bool,
}

impl TextCollector {
    fn push_text(&mut self, text: &str, mode: HtmlWhitespaceMode) {
        match mode {
            HtmlWhitespaceMode::Preserve => {
                self.buffer.push_str(text);
                self.refresh_state_from_text(text);
            }
            HtmlWhitespaceMode::Collapse => {
                for ch in text.chars() {
                    if ch.is_whitespace() {
                        if !self.last_was_space {
                            self.buffer.push(' ');
                            self.last_was_space = true;
                        }
                        self.last_was_line_break = false;
                    } else {
                        self.buffer.push(ch);
                        self.last_was_space = false;
                        self.last_was_line_break = false;
                    }
                }
            }
            HtmlWhitespaceMode::PreserveLineBreaks => {
                for ch in text.chars() {
                    if ch == '\n' || ch == '\r' {
                        if !self.last_was_line_break {
                            self.buffer.push('\n');
                        }
                        self.last_was_space = false;
                        self.last_was_line_break = true;
                    } else if ch.is_whitespace() {
                        if !self.last_was_space {
                            self.buffer.push(' ');
                            self.last_was_space = true;
                        }
                        self.last_was_line_break = false;
                    } else {
                        self.buffer.push(ch);
                        self.last_was_space = false;
                        self.last_was_line_break = false;
                    }
                }
            }
        }
    }

    fn push_line_break(&mut self) {
        if !self.buffer.ends_with('\n') {
            self.buffer.push('\n');
        }
        self.last_was_space = false;
        self.last_was_line_break = true;
    }

    fn refresh_state_from_text(&mut self, text: &str) {
        if let Some(last) = text.chars().last() {
            self.last_was_space = last == ' ' || last == '\t';
            self.last_was_line_break = last == '\n' || last == '\r';
        }
    }
}

pub(crate) fn collect_text_content(element: ElementRef, context: &HtmlRenderContext) -> String {
    let mut collector = TextCollector::default();
    for child in element.children() {
        collect_text_recursive(child, &mut collector, context.whitespace_mode);
    }
    let normalized = collector.buffer.nfc().collect::<String>();
    match context.whitespace_mode {
        HtmlWhitespaceMode::Preserve => normalized,
        _ => normalized.trim().to_string(),
    }
}

pub(crate) fn collect_immediate_inline_text_content(
    element: ElementRef,
    context: &HtmlRenderContext,
) -> String {
    let mut collector = TextCollector::default();
    for child in element.children() {
        collect_inline_text_recursive(child, &mut collector, context.whitespace_mode);
    }
    let normalized = collector.buffer.nfc().collect::<String>();
    match context.whitespace_mode {
        HtmlWhitespaceMode::Preserve => normalized,
        _ => normalized.trim().to_string(),
    }
}

pub(crate) fn resolve_text_direction(
    context: &HtmlRenderContext,
    text: &str,
) -> (TextDirection, ValueOrigin) {
    if context.text_direction == TextDirection::TopToBottom {
        return (context.text_direction, context.text_direction_origin);
    }

    if context.auto_direction
        && let Some(inferred) = infer_text_direction_from_text(text)
    {
        return (inferred, ValueOrigin::Estimated);
    }

    (context.text_direction, context.text_direction_origin)
}

fn collect_text_recursive(
    node: NodeRef<Node>,
    collector: &mut TextCollector,
    whitespace_mode: HtmlWhitespaceMode,
) {
    match node.value() {
        Node::Text(text) => collector.push_text(text, whitespace_mode),
        Node::Element(_) => {
            let Some(element) = ElementRef::wrap(node) else {
                return;
            };
            let tag = element.value().name();
            if SKIP_ELEMENTS.contains(&tag) || is_hidden_element(element) {
                return;
            }
            if WALK_HANDLED_ELEMENTS.contains(&tag) || structural_role(element).is_some() {
                return;
            }
            if tag == "br" {
                collector.push_line_break();
                return;
            }
            let child_whitespace_mode = derive_whitespace_mode(element, whitespace_mode);
            for child in node.children() {
                collect_text_recursive(child, collector, child_whitespace_mode);
            }
        }
        _ => {}
    }
}

fn collect_inline_text_recursive(
    node: NodeRef<Node>,
    collector: &mut TextCollector,
    whitespace_mode: HtmlWhitespaceMode,
) {
    match node.value() {
        Node::Text(text) => collector.push_text(text, whitespace_mode),
        Node::Element(_) => {
            let Some(element) = ElementRef::wrap(node) else {
                return;
            };
            let tag = element.value().name();
            if SKIP_ELEMENTS.contains(&tag) || is_hidden_element(element) {
                return;
            }
            if tag == "br" {
                collector.push_line_break();
                return;
            }
            if WALK_HANDLED_ELEMENTS.contains(&tag)
                || SELF_EMITTING_ELEMENTS.contains(&tag)
                || GENERIC_TEXT_CONTAINER_ELEMENTS.contains(&tag)
                || structural_role(element).is_some()
            {
                return;
            }

            let child_whitespace_mode = derive_whitespace_mode(element, whitespace_mode);
            for child in node.children() {
                collect_inline_text_recursive(child, collector, child_whitespace_mode);
            }
        }
        _ => {}
    }
}

fn infer_text_direction_from_text(text: &str) -> Option<TextDirection> {
    let mut saw_rtl = false;
    let mut saw_ltr = false;

    for ch in text.chars() {
        if is_rtl_text(ch as u32) {
            saw_rtl = true;
        } else if ch.is_alphabetic() {
            saw_ltr = true;
        }
    }

    match (saw_ltr, saw_rtl) {
        (true, true) => Some(TextDirection::Mixed),
        (true, false) => Some(TextDirection::LeftToRight),
        (false, true) => Some(TextDirection::RightToLeft),
        (false, false) => None,
    }
}
