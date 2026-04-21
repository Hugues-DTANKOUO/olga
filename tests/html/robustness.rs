use olga::model::*;

use crate::support::html::{decode_html, text_primitives};

#[test]
fn decode_extreme_colspan_clamped() {
    let html = r#"<html><body>
        <table>
            <tr><td colspan="999999">Wide</td></tr>
            <tr><td>Normal</td></tr>
        </table>
    </body></html>"#;
    let result = decode_html(html);
    assert!(!result.primitives.is_empty());
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.kind == olga::error::WarningKind::TruncatedContent)
    );
    let normal = result
        .primitives
        .iter()
        .find(|p| p.text_content() == Some("Normal"))
        .unwrap();
    assert!(
        normal
            .hints
            .iter()
            .any(|h| matches!(h.kind, HintKind::TableCell { row: 1, col: 0, .. }))
    );
}

#[test]
fn decode_extreme_rowspan_clamped() {
    let html = r#"<html><body>
        <table>
            <tr><td rowspan="1000000">Tall</td><td>B</td></tr>
            <tr><td>C</td></tr>
        </table>
    </body></html>"#;
    let result = decode_html(html);
    assert!(!result.primitives.is_empty());
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.kind == olga::error::WarningKind::TruncatedContent)
    );
}

#[test]
fn decode_deeply_nested_divs_no_stack_overflow() {
    let mut html = String::from("<html><body>");
    for _ in 0..100 {
        html.push_str("<div>");
    }
    html.push_str("<p>Deep</p>");
    for _ in 0..100 {
        html.push_str("</div>");
    }
    html.push_str("</body></html>");
    let result = decode_html(&html);
    let texts = text_primitives(&result);
    assert!(texts.contains(&"Deep"));
}

#[test]
fn decode_extreme_nesting_truncated_no_crash() {
    let mut html = String::from("<html><body>");
    for _ in 0..500 {
        html.push_str("<div>");
    }
    html.push_str("<p>TooDeep</p>");
    for _ in 0..500 {
        html.push_str("</div>");
    }
    html.push_str("</body></html>");
    let result = decode_html(&html);
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.kind == olga::error::WarningKind::TruncatedContent)
    );
}

#[test]
fn decode_unicode_nfc_normalization() {
    let html = "<html><body><p>caf\u{0065}\u{0301}</p></body></html>";
    let result = decode_html(html);
    let text = result.primitives[0].text_content().unwrap();
    assert_eq!(text, "caf\u{00e9}");
}

#[test]
fn decode_empty_table_no_crash() {
    let html = "<html><body><table><tr></tr></table></body></html>";
    let result = decode_html(html);
    assert!(result.primitives.is_empty());
}
