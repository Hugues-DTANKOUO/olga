use olga::formats::html::{HtmlDecoder, HtmlDecoderMode};
use olga::model::*;
use olga::traits::FormatDecoder;

use crate::support::html::{decode_html, text_primitives};

#[test]
fn decode_full_semantic_html() {
    let html = r#"<!DOCTYPE html>
<html lang="en">
<head><title>Test Document</title></head>
<body>
    <h1>Main Title</h1>
    <p>First paragraph with <strong>bold</strong> and <em>italic</em> text.</p>
    <h2>Section Two</h2>
    <p>Second paragraph.</p>
    <blockquote>A famous quote.</blockquote>
    <pre><code>fn main() { println!("Hello"); }</code></pre>
</body>
</html>"#;

    let result = decode_html(html);
    assert_eq!(result.metadata.format, DocumentFormat::Html);
    assert_eq!(result.metadata.title, Some("Test Document".to_string()));
    assert_eq!(result.metadata.page_count, 1);

    let texts = text_primitives(&result);
    assert!(texts.iter().any(|t| t.contains("Main Title")));
    assert!(texts.iter().any(|t| t.contains("bold")));
    assert!(texts.iter().any(|t| t.contains("famous quote")));
    assert!(texts.iter().any(|t| t.contains("fn main()")));
}

#[test]
fn decode_metadata_only() {
    let html =
        r#"<html><head><title>Metadata Test</title></head><body><p>Content</p></body></html>"#;
    let decoder = HtmlDecoder::new();
    let meta = decoder.metadata(html.as_bytes()).unwrap();
    assert_eq!(meta.format, DocumentFormat::Html);
    assert_eq!(meta.title, Some("Metadata Test".to_string()));
    assert_eq!(meta.page_count, 1);
    assert!(meta.is_processable());
}

#[test]
fn decode_exposes_static_rendered_subset_mode() {
    let decoder = HtmlDecoder::new();
    assert_eq!(decoder.mode(), HtmlDecoderMode::StaticRenderedSubsetDomFlow);
}

#[test]
fn decode_html_contract_uses_dom_flow_synthetic_geometry() {
    let result =
        decode_html(r#"<html><body><p>Alpha</p><div role="paragraph">Beta</div></body></html>"#);

    assert_eq!(
        result.metadata.page_count_provenance.origin,
        ValueOrigin::Synthetic
    );
    assert_eq!(
        result.metadata.page_count_provenance.basis,
        PaginationBasis::SingleLogicalPage
    );
    assert!(result.primitives.iter().all(|primitive| {
        primitive.geometry_provenance.origin == ValueOrigin::Synthetic
            && primitive.geometry_provenance.space == GeometrySpace::DomFlow
    }));
}

#[test]
fn decode_lang_from_html_propagates_to_all() {
    let html = r#"<html lang="de-DE"><body>
        <h1>Titel</h1>
        <p>Absatz</p>
    </body></html>"#;

    let result = decode_html(html);
    for prim in &result.primitives {
        if let PrimitiveKind::Text { lang, .. } = &prim.kind {
            assert_eq!(lang.as_deref(), Some("de-DE"));
        }
    }
}

#[test]
fn decode_lang_element_override() {
    let html = r#"<html lang="en"><body>
        <p>English paragraph</p>
        <p lang="fr">Paragraphe français</p>
    </body></html>"#;

    let result = decode_html(html);
    let en_prim = result
        .primitives
        .iter()
        .find(|p| p.text_content().is_some_and(|t| t.contains("English")))
        .unwrap();
    let fr_prim = result
        .primitives
        .iter()
        .find(|p| p.text_content().is_some_and(|t| t.contains("français")))
        .unwrap();

    match &en_prim.kind {
        PrimitiveKind::Text { lang, .. } => assert_eq!(lang.as_deref(), Some("en")),
        _ => panic!("Expected Text"),
    }
    match &fr_prim.kind {
        PrimitiveKind::Text { lang, .. } => assert_eq!(lang.as_deref(), Some("fr")),
        _ => panic!("Expected Text"),
    }
}

#[test]
fn decode_hidden_content_is_skipped_from_output() {
    // Visibility policy (see render::is_hidden_element):
    //   - `hidden` attribute            → hide
    //   - inline style display:none     → hide
    //   - inline style visibility:hidden → hide
    //   - inline style content-visibility:hidden → hide
    //   - aria-hidden="true"            → NOT a visibility signal (WAI-ARIA § 6.8);
    //                                      removes from the accessibility tree but
    //                                      has no effect on visual rendering.
    //                                      Matches pandoc / Tika / Readability.
    let html = r#"<html><body>
        <p>Visible <span hidden>secret</span> text</p>
        <p style="display:none">Gone</p>
        <p>Before<span aria-hidden="true">masked</span>After</p>
    </body></html>"#;

    let result = decode_html(html);
    let texts = text_primitives(&result);

    assert!(texts.contains(&"Visible text"));
    // aria-hidden content IS surfaced — it's part of the visual document.
    assert!(texts.iter().any(|text| text.contains("BeforemaskedAfter")));
    assert!(!texts.iter().any(|text| text.contains("secret")));
    assert!(!texts.iter().any(|text| text.contains("Gone")));
}

#[test]
fn decode_html_direction_and_writing_mode_are_propagated() {
    let html = r#"<html dir="rtl"><body>
        <p>مرحبا بالعالم</p>
        <section style="writing-mode: vertical-rl"><p>Vertical title</p></section>
    </body></html>"#;

    let result = decode_html(html);
    let rtl = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("مرحبا بالعالم"))
        .unwrap();
    let vertical = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Vertical title"))
        .unwrap();

    match &rtl.kind {
        PrimitiveKind::Text {
            text_direction,
            text_direction_origin,
            ..
        } => {
            assert_eq!(*text_direction, TextDirection::RightToLeft);
            assert_eq!(*text_direction_origin, ValueOrigin::Reconstructed);
        }
        _ => panic!("Expected text primitive"),
    }

    match &vertical.kind {
        PrimitiveKind::Text {
            text_direction,
            text_direction_origin,
            ..
        } => {
            assert_eq!(*text_direction, TextDirection::TopToBottom);
            assert_eq!(*text_direction_origin, ValueOrigin::Reconstructed);
        }
        _ => panic!("Expected text primitive"),
    }
}

#[test]
fn decode_html_white_space_styles_are_respected() {
    let html = "<html><body>\
        <p style=\"white-space: pre\">  line1\n  line2</p>\
        <p style=\"white-space: pre-line\">A   B\nC    D</p>\
    </body></html>";

    let result = decode_html(html);
    let preserved = result
        .primitives
        .iter()
        .find(|primitive| {
            primitive
                .text_content()
                .is_some_and(|text| text.contains("line1"))
        })
        .unwrap();
    let pre_line = result
        .primitives
        .iter()
        .find(|primitive| {
            primitive
                .text_content()
                .is_some_and(|text| text.contains("A"))
        })
        .unwrap();

    assert_eq!(preserved.text_content(), Some("  line1\n  line2"));
    assert_eq!(pre_line.text_content(), Some("A B\nC D"));
}

#[test]
fn decode_extensions() {
    let decoder = HtmlDecoder::new();
    let exts = decoder.supported_extensions();
    assert!(exts.contains(&"html"));
    assert!(exts.contains(&"htm"));
    assert!(exts.contains(&"xhtml"));
}
