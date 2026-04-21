use olga::model::*;

use crate::support::html::{decode_html, text_primitives};

#[test]
fn decode_unsupported_layout_styles_warn_once() {
    let html = r#"<html><body>
        <div style="display:flex"><p>A</p></div>
        <div style="display:flex"><p>B</p></div>
        <div style="position:absolute"><p>C</p></div>
    </body></html>"#;

    let result = decode_html(html);
    let flex_warnings = result
        .warnings
        .iter()
        .filter(|warning| warning.message.contains("display:flex"))
        .count();
    let position_warnings = result
        .warnings
        .iter()
        .filter(|warning| warning.message.contains("position:absolute"))
        .count();

    assert_eq!(flex_warnings, 1);
    assert_eq!(position_warnings, 1);
}

#[test]
fn decode_div_soup_extracts_text() {
    let html = r#"<html><body>
        <div class="container">
            <div class="row">
                <div class="col">Some text in divs</div>
            </div>
        </div>
    </body></html>"#;

    let result = decode_html(html);
    let texts = text_primitives(&result);
    assert!(texts.iter().any(|text| text.contains("Some text in divs")));
    assert!(result.warnings.iter().any(|warning| {
        warning.kind == olga::error::WarningKind::HeuristicInference
            && warning.message.contains("<div>")
            && warning.message.contains("generic container fallback")
    }));
}

#[test]
fn decode_generic_container_does_not_duplicate_nested_paragraphs() {
    let html = r#"<html><body>
        <div class="wrapper">
            <p>Nested paragraph</p>
        </div>
    </body></html>"#;

    let result = decode_html(html);
    let texts = text_primitives(&result);
    assert_eq!(texts, vec!["Nested paragraph"]);
}

#[test]
fn decode_role_based_heading_and_paragraph_from_divs() {
    let html = r#"<html><body>
        <div role="heading" aria-level="3">Role heading</div>
        <div role="paragraph">Role paragraph</div>
    </body></html>"#;

    let result = decode_html(html);
    let heading = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Role heading"))
        .unwrap();
    let paragraph = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Role paragraph"))
        .unwrap();

    assert!(
        heading
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::Heading { level: 3 }))
    );
    assert!(
        paragraph
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::Paragraph))
    );
}

#[test]
fn decode_aside_emits_sidebar_hint_when_used_as_text_container() {
    let html = "<html><body><aside>Operational note</aside></body></html>";
    let result = decode_html(html);
    let aside = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Operational note"))
        .unwrap();
    assert!(
        aside
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::Sidebar))
    );
}

#[test]
fn decode_html_email_skips_non_content() {
    let html = r#"<!DOCTYPE html>
<html>
<head>
    <style>body { font-family: Arial; }</style>
</head>
<body>
    <table><tr><td>
        <h1>Newsletter</h1>
        <p>Important update for you.</p>
        <img src="tracking-pixel.gif" alt="" width="1" height="1">
    </td></tr></table>
    <script>console.log("tracker");</script>
</body>
</html>"#;

    let result = decode_html(html);
    let texts = text_primitives(&result);
    assert!(texts.iter().any(|t| t.contains("Newsletter")));
    assert!(texts.iter().any(|t| t.contains("Important update")));
    assert!(!texts.iter().any(|t| t.contains("tracker")));
}

#[test]
fn decode_empty_html_no_crash() {
    let result = decode_html("");
    assert!(result.primitives.is_empty());
}

#[test]
fn decode_fragment_no_crash() {
    let result = decode_html("<p>Just a fragment</p>");
    assert_eq!(result.primitives.len(), 1);
}

#[test]
fn decode_y_positions_increase() {
    let html = r#"<html><body>
        <h1>A</h1>
        <p>B</p>
        <p>C</p>
    </body></html>"#;
    let result = decode_html(html);
    for window in result.primitives.windows(2) {
        assert!(window[0].bbox.y <= window[1].bbox.y);
    }
}
