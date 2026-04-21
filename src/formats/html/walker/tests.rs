use scraper::ElementRef;

use super::*;
use crate::formats::html::render::is_hidden_element;
use crate::formats::html::text::heading_font_size;
use crate::model::{TextDirection, ValueOrigin};

#[test]
fn infer_text_direction_detects_mixed_content() {
    assert_eq!(
        crate::formats::html::text::resolve_text_direction(
            &HtmlRenderContext {
                auto_direction: true,
                ..HtmlRenderContext::default()
            },
            "hello שלום"
        ),
        (TextDirection::Mixed, ValueOrigin::Estimated)
    );
}

#[test]
fn hidden_style_detection_recognizes_display_none() {
    let html = scraper::Html::parse_fragment(r#"<span style="display:none">x</span>"#);
    let element = html
        .root_element()
        .first_child()
        .and_then(ElementRef::wrap)
        .unwrap();
    assert!(is_hidden_element(element));
}

#[test]
fn heading_font_sizes() {
    assert_eq!(heading_font_size(1), 24.0);
    assert_eq!(heading_font_size(6), 11.0);
}

#[test]
fn text_collector_preserves_line_breaks_for_pre_line() {
    let html = scraper::Html::parse_fragment(
        r#"<div style="white-space:pre-line">A   B<br/>C    D</div>"#,
    );
    let element = html
        .root_element()
        .first_child()
        .and_then(ElementRef::wrap)
        .unwrap();
    let context = crate::formats::html::render::derive_render_context(
        element,
        &HtmlRenderContext::default(),
        &mut WalkState::new(),
        &mut Vec::new(),
    )
    .unwrap();
    let text = crate::formats::html::text::collect_text_content(element, &context);
    assert_eq!(text, "A B\nC D");
}
