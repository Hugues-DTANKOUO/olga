use olga::model::*;

use crate::support::html::{decode_html, text_primitives};

#[test]
fn decode_table_with_thead_tbody() {
    let html = r#"<html><body>
        <table>
            <thead>
                <tr><th>Name</th><th>Role</th><th>Location</th></tr>
            </thead>
            <tbody>
                <tr><td>Alice</td><td>Engineer</td><td>Paris</td></tr>
                <tr><td>Bob</td><td>Designer</td><td>London</td></tr>
            </tbody>
        </table>
    </body></html>"#;

    let result = decode_html(html);
    assert_eq!(result.primitives.len(), 9);

    let headers: Vec<_> = result
        .primitives
        .iter()
        .filter(|p| {
            p.hints
                .iter()
                .any(|h| matches!(h.kind, HintKind::TableHeader { .. }))
        })
        .collect();
    assert_eq!(headers.len(), 3);
}

#[test]
fn decode_table_colspan_rowspan_complex() {
    let html = r#"<html><body>
        <table>
            <tr><td colspan="3">Title Row</td></tr>
            <tr><td rowspan="2">Left</td><td>Center1</td><td>Right1</td></tr>
            <tr><td>Center2</td><td>Right2</td></tr>
        </table>
    </body></html>"#;

    let result = decode_html(html);

    let title = result
        .primitives
        .iter()
        .find(|p| p.text_content() == Some("Title Row"))
        .unwrap();
    assert!(title.hints.iter().any(|h| matches!(
        h.kind,
        HintKind::TableCell {
            row: 0,
            col: 0,
            colspan: 3,
            rowspan: 1
        }
    )));

    let c2 = result
        .primitives
        .iter()
        .find(|p| p.text_content() == Some("Center2"))
        .unwrap();
    assert!(
        c2.hints
            .iter()
            .any(|h| matches!(h.kind, HintKind::TableCell { row: 2, col: 1, .. }))
    );

    let r2 = result
        .primitives
        .iter()
        .find(|p| p.text_content() == Some("Right2"))
        .unwrap();
    assert!(
        r2.hints
            .iter()
            .any(|h| matches!(h.kind, HintKind::TableCell { row: 2, col: 2, .. }))
    );
}

#[test]
fn decode_deeply_nested_list() {
    let html = r#"<html><body>
        <ul>
            <li>Level 0
                <ul>
                    <li>Level 1
                        <ol>
                            <li>Level 2 ordered</li>
                        </ol>
                    </li>
                </ul>
            </li>
        </ul>
    </body></html>"#;

    let result = decode_html(html);
    let texts = text_primitives(&result);
    assert!(texts.iter().any(|t| t.contains("Level 0")));
    assert!(texts.iter().any(|t| t.contains("Level 1")));
    assert!(texts.iter().any(|t| t.contains("Level 2 ordered")));

    let l2 = result
        .primitives
        .iter()
        .find(|p| {
            p.text_content()
                .is_some_and(|t| t.contains("Level 2 ordered"))
        })
        .unwrap();
    assert!(l2.hints.iter().any(|h| matches!(
        h.kind,
        HintKind::ListItem {
            depth: 2,
            ordered: true,
            ..
        }
    )));
}

#[test]
fn decode_aria_list_structure_from_divs() {
    let html = r#"<html><body>
        <div role="list">
            <div role="listitem">Alpha</div>
            <div role="listitem">
                Beta
                <div role="list">
                    <div role="listitem">Nested</div>
                </div>
            </div>
        </div>
    </body></html>"#;

    let result = decode_html(html);

    let alpha = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Alpha"))
        .unwrap();
    let beta = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Beta"))
        .unwrap();
    let nested = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Nested"))
        .unwrap();

    assert!(alpha.hints.iter().any(|hint| matches!(
        hint.kind,
        HintKind::ListItem {
            depth: 0,
            ordered: false,
            ..
        }
    )));
    assert!(beta.hints.iter().any(|hint| matches!(
        hint.kind,
        HintKind::ListItem {
            depth: 0,
            ordered: false,
            ..
        }
    )));
    assert!(nested.hints.iter().any(|hint| matches!(
        hint.kind,
        HintKind::ListItem {
            depth: 1,
            ordered: false,
            ..
        }
    )));
}

#[test]
fn decode_aria_table_structure_from_divs() {
    let html = r#"<html><body>
        <div role="table">
            <div role="rowgroup">
                <div role="row">
                    <div role="columnheader">Name</div>
                    <div role="columnheader">Role</div>
                </div>
            </div>
            <div role="rowgroup">
                <div role="row">
                    <div role="cell">Alice</div>
                    <div role="cell">Engineer</div>
                </div>
                <div role="row">
                    <div role="cell">Bob</div>
                    <div role="cell">Designer</div>
                </div>
            </div>
        </div>
    </body></html>"#;

    let result = decode_html(html);

    let name = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Name"))
        .unwrap();
    let engineer = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Engineer"))
        .unwrap();
    let designer = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Designer"))
        .unwrap();

    assert!(
        name.hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::TableHeader { col: 0 }))
    );
    assert!(engineer.hints.iter().any(|hint| matches!(
        hint.kind,
        HintKind::TableCell {
            row: 1,
            col: 1,
            rowspan: 1,
            colspan: 1
        }
    )));
    assert!(designer.hints.iter().any(|hint| matches!(
        hint.kind,
        HintKind::TableCell {
            row: 2,
            col: 1,
            rowspan: 1,
            colspan: 1
        }
    )));
}

#[test]
fn decode_css_display_list_items_from_divs() {
    let html = r#"<html><body>
        <div class="list">
            <div style="display:list-item">Alpha</div>
            <div style="display:list-item">Beta</div>
        </div>
    </body></html>"#;

    let result = decode_html(html);
    let alpha = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Alpha"))
        .unwrap();
    let beta = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Beta"))
        .unwrap();

    assert!(alpha.hints.iter().any(|hint| matches!(
        hint.kind,
        HintKind::ListItem {
            depth: 0,
            ordered: false,
            ..
        }
    )));
    assert!(beta.hints.iter().any(|hint| matches!(
        hint.kind,
        HintKind::ListItem {
            depth: 0,
            ordered: false,
            ..
        }
    )));
    assert!(result.warnings.iter().any(|warning| {
        warning.kind == olga::error::WarningKind::HeuristicInference
            && warning.message.contains("display:list-item")
    }));
}

#[test]
fn decode_css_display_list_items_with_decimal_markers_are_ordered() {
    let html = r#"<html><body>
        <div style="list-style-type: decimal">
            <div style="display:list-item">Alpha</div>
            <div style="display:list-item">Beta</div>
        </div>
    </body></html>"#;

    let result = decode_html(html);
    let alpha = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Alpha"))
        .unwrap();
    let beta = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Beta"))
        .unwrap();

    assert!(alpha.hints.iter().any(|hint| matches!(
        hint.kind,
        HintKind::ListItem {
            depth: 0,
            ordered: true,
            ..
        }
    )));
    assert!(beta.hints.iter().any(|hint| matches!(
        hint.kind,
        HintKind::ListItem {
            depth: 0,
            ordered: true,
            ..
        }
    )));
}

#[test]
fn decode_css_display_table_structure_from_divs() {
    let html = r#"<html><body>
        <div style="display:table">
            <div style="display:table-header-group">
                <div style="display:table-row">
                    <div style="display:table-cell">Name</div>
                    <div style="display:table-cell">Role</div>
                </div>
            </div>
            <div style="display:table-row-group">
                <div style="display:table-row">
                    <div style="display:table-cell">Alice</div>
                    <div style="display:table-cell">Engineer</div>
                </div>
            </div>
        </div>
    </body></html>"#;

    let result = decode_html(html);
    let name = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Name"))
        .unwrap();
    let engineer = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Engineer"))
        .unwrap();

    assert!(
        name.hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::TableHeader { col: 0 }))
    );
    assert!(engineer.hints.iter().any(|hint| matches!(
        hint.kind,
        HintKind::TableCell {
            row: 1,
            col: 1,
            rowspan: 1,
            colspan: 1
        }
    )));
    assert!(result.warnings.iter().any(|warning| {
        warning.kind == olga::error::WarningKind::HeuristicInference
            && warning.message.contains("display:table")
    }));
}

#[test]
fn decode_native_table_caption_bottom_after_cells() {
    let html = r#"<html><body>
        <table>
            <caption style="caption-side: bottom">Quarterly headcount</caption>
            <tr><td>Alice</td></tr>
        </table>
    </body></html>"#;

    let result = decode_html(html);
    assert_eq!(
        text_primitives(&result),
        vec!["Alice", "Quarterly headcount"]
    );
}

#[test]
fn decode_css_display_table_caption_bottom_after_cells() {
    let html = r#"<html><body>
        <div style="display:table">
            <div style="display:table-caption; caption-side: bottom">Team roster</div>
            <div style="display:table-row-group">
                <div style="display:table-row">
                    <div style="display:table-cell">Alice</div>
                </div>
            </div>
        </div>
    </body></html>"#;

    let result = decode_html(html);
    assert_eq!(text_primitives(&result), vec!["Alice", "Team roster"]);
}

// ---------------------------------------------------------------------------
// <form> handling — regression guard against the faq.html anomaly where
// labels were concatenated with option text into one undifferentiated block
// and a leading <h3> surfaced after the form's controls.
// ---------------------------------------------------------------------------

#[test]
fn decode_form_preserves_dom_order_heading_first() {
    // Regression: previously the generic-container fallback emitted the form's
    // inline text as a single paragraph FIRST and walked the <h3> afterwards,
    // placing the heading at the end of the form block. With dedicated form
    // handling, DOM order is preserved.
    let html = r#"<html><body>
        <form>
            <h3>Contact form</h3>
            <label for="name">Name</label>
            <input type="text" id="name">
            <button type="submit">Send</button>
        </form>
    </body></html>"#;

    let result = decode_html(html);
    let texts = text_primitives(&result);
    assert_eq!(texts, vec!["Contact form", "Name", "Send"]);
}

#[test]
fn decode_form_labels_emit_as_separate_blocks() {
    // Regression: previously all label text was concatenated into one block
    // like "Partner ID Contact name Email address...". Each label must emit
    // as its own paragraph primitive.
    let html = r#"<html><body>
        <form>
            <label for="a">Partner ID</label>
            <input id="a">
            <label for="b">Contact name</label>
            <input id="b">
            <label for="c">Email address</label>
            <input id="c">
        </form>
    </body></html>"#;

    let result = decode_html(html);
    let texts = text_primitives(&result);
    assert!(texts.contains(&"Partner ID"));
    assert!(texts.contains(&"Contact name"));
    assert!(texts.contains(&"Email address"));
    // No primitive should contain the concatenation.
    assert!(!texts.iter().any(|t| t.contains("Partner ID Contact name")));
}

#[test]
fn decode_select_options_emit_as_list_items() {
    // Regression: previously the <select>'s options collapsed into the
    // preceding label's text. Options must emit as separate list-item
    // primitives, which is semantically closest to a listbox (HTML §4.10.7
    // maps <select> to ARIA listbox, <option> to ARIA option).
    let html = r#"<html><body>
        <form>
            <label for="t">Inquiry type</label>
            <select id="t" name="type">
                <option>Order or shipment</option>
                <option>Billing or payment</option>
                <option>Other</option>
            </select>
        </form>
    </body></html>"#;

    let result = decode_html(html);
    let texts = text_primitives(&result);
    assert_eq!(
        texts,
        vec![
            "Inquiry type",
            "Order or shipment",
            "Billing or payment",
            "Other",
        ]
    );

    for option_text in ["Order or shipment", "Billing or payment", "Other"] {
        let option = result
            .primitives
            .iter()
            .find(|primitive| primitive.text_content() == Some(option_text))
            .unwrap_or_else(|| panic!("missing option: {option_text}"));
        assert!(
            option.hints.iter().any(|hint| matches!(
                hint.kind,
                HintKind::ListItem {
                    depth: 0,
                    ordered: false,
                    ..
                }
            )),
            "option {option_text:?} should be a list item"
        );
    }

    // The label stays a paragraph, not a list item.
    let label = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Inquiry type"))
        .unwrap();
    assert!(
        label
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::Paragraph)),
        "label should be a paragraph, not a list item"
    );
}

#[test]
fn decode_form_label_wrapping_checkbox_and_link_emits_full_text() {
    // Real-world pattern: the consent checkbox's <label> wraps the checkbox
    // <input> AND an inline <a> for the privacy policy link. The entire
    // sentence must surface as one block so the sentence is not fragmented.
    let html = r#"<html><body>
        <form>
            <label>
                <input type="checkbox" name="consent">
                I consent to the <a href="https://example/privacy">Privacy Policy</a>.
            </label>
            <button type="submit">Submit inquiry</button>
        </form>
    </body></html>"#;

    let result = decode_html(html);
    let texts = text_primitives(&result);
    assert!(texts.contains(&"I consent to the Privacy Policy."));
    // And the link text must not appear as a fragmented standalone primitive.
    assert!(!texts.contains(&"Privacy Policy"));
    assert!(texts.contains(&"Submit inquiry"));
}

#[test]
fn decode_input_placeholder_surfaces_as_paragraph() {
    // Author placeholder text is human-readable guidance; surface it rather
    // than dropping the author's authored content.
    let html = r#"<html><body>
        <form>
            <label for="id">Partner ID</label>
            <input type="text" id="id" placeholder="e.g. KPS-P-04412">
        </form>
    </body></html>"#;

    let result = decode_html(html);
    let texts = text_primitives(&result);
    assert_eq!(texts, vec!["Partner ID", "e.g. KPS-P-04412"]);
}

#[test]
fn decode_hidden_and_submit_inputs_emit_nothing() {
    // Non-textual inputs (hidden, checkbox, radio, submit) carry no visible
    // author content of their own — dropping them avoids polluting the
    // primitives stream with empty or machine-only metadata.
    let html = r#"<html><body>
        <form>
            <input type="hidden" name="csrf" value="secret-token">
            <input type="checkbox" name="agree">
            <input type="submit" value="Go">
        </form>
    </body></html>"#;

    let result = decode_html(html);
    let texts = text_primitives(&result);
    assert!(
        texts.is_empty(),
        "non-text inputs should not emit, got {texts:?}"
    );
}

// ---------------------------------------------------------------------------
// HTML5 disclosure widgets (`<details>` / `<summary>`)
// ---------------------------------------------------------------------------
//
// These are a common FAQ / expandable-section pattern. Both the summary and
// the body are visible content regardless of open/closed state. Pandoc, Tika,
// Readability and html2text all surface them; a prior version of this
// walker dropped them because the allowlist didn't include either tag.

#[test]
fn decode_details_summary_surfaces_both_question_and_answer() {
    let html = r#"<html><body>
        <details>
            <summary>What is the minimum order quantity?</summary>
            <p>The standard MOQ is 5 cases of 48 vials.</p>
        </details>
    </body></html>"#;

    let result = decode_html(html);
    let texts = text_primitives(&result);

    assert!(
        texts.contains(&"What is the minimum order quantity?"),
        "summary text must surface, got {texts:?}"
    );
    assert!(
        texts.contains(&"The standard MOQ is 5 cases of 48 vials."),
        "answer paragraph must surface, got {texts:?}"
    );
}

#[test]
fn decode_details_summary_preserves_dom_order() {
    // summary should come before the answer paragraph in the primitives
    // stream, matching how screen readers and text extractors announce them.
    let html = r#"<html><body>
        <details>
            <summary>Q1</summary>
            <p>A1</p>
        </details>
        <details>
            <summary>Q2</summary>
            <p>A2</p>
        </details>
    </body></html>"#;

    let result = decode_html(html);
    let texts = text_primitives(&result);

    let q1 = texts.iter().position(|t| *t == "Q1").expect("Q1 emitted");
    let a1 = texts.iter().position(|t| *t == "A1").expect("A1 emitted");
    let q2 = texts.iter().position(|t| *t == "Q2").expect("Q2 emitted");
    let a2 = texts.iter().position(|t| *t == "A2").expect("A2 emitted");

    assert!(
        q1 < a1 && a1 < q2 && q2 < a2,
        "order must be Q1,A1,Q2,A2 — got {texts:?}"
    );
}

#[test]
fn decode_summary_is_emitted_as_bold_paragraph_not_heading() {
    // We deliberately DO NOT fabricate a heading level for <summary> — it
    // labels its disclosure but doesn't carry document-outline semantics.
    // Emitting it bold preserves the visual-weight hint authors give it in
    // CSS without imposing a synthetic H-level on the assembler.
    let html = r#"<html><body>
        <details>
            <summary>Bold label</summary>
            <p>Body text.</p>
        </details>
    </body></html>"#;

    let result = decode_html(html);
    let summary = result
        .primitives
        .iter()
        .find(|p| p.text_content() == Some("Bold label"))
        .expect("summary primitive emitted");

    // Must carry a Paragraph hint, not a Heading hint.
    assert!(
        summary
            .hints
            .iter()
            .any(|h| matches!(h.kind, HintKind::Paragraph)),
        "summary should carry Paragraph hint, got {:?}",
        summary.hints,
    );
    assert!(
        !summary
            .hints
            .iter()
            .any(|h| matches!(h.kind, HintKind::Heading { .. })),
        "summary must not invent a heading level, got {:?}",
        summary.hints,
    );

    // And must be bold (visual-weight author intent).
    match &summary.kind {
        PrimitiveKind::Text { is_bold, .. } => {
            assert!(*is_bold, "summary should be emitted as bold text");
        }
        _ => panic!("summary should be a Text primitive"),
    }
}

#[test]
fn decode_details_with_rich_inline_content_in_summary() {
    // Summary may contain inline elements — <strong>, <a>, <code>. The text
    // collector flattens them into the summary's visible string.
    let html = r#"<html><body>
        <details>
            <summary>Price of <strong>Vertexidine</strong> (<code>DIN 02-514-883</code>)</summary>
            <p>Details here.</p>
        </details>
    </body></html>"#;

    let result = decode_html(html);
    let texts = text_primitives(&result);

    assert!(
        texts
            .iter()
            .any(|t| t.contains("Vertexidine") && t.contains("DIN 02-514-883")),
        "inline content in summary should flatten into visible text, got {texts:?}"
    );
}

// ---------------------------------------------------------------------------
// ARIA-hidden policy
// ---------------------------------------------------------------------------

#[test]
fn decode_aria_hidden_content_still_surfaces() {
    // Regression for FAQ internal-note drop. aria-hidden="true" removes the
    // element from the accessibility tree (screen readers skip) but does NOT
    // hide it visually. Pandoc, Tika and Readability all surface it; we
    // used to drop it.
    let html = r#"<html><body>
        <div aria-hidden="true">[INTERNAL: deploy 2026-04-15]</div>
        <p>Visible paragraph.</p>
    </body></html>"#;

    let result = decode_html(html);
    let texts = text_primitives(&result);

    assert!(
        texts
            .iter()
            .any(|t| t.contains("[INTERNAL: deploy 2026-04-15]")),
        "aria-hidden content must still surface, got {texts:?}"
    );
    assert!(texts.contains(&"Visible paragraph."));
}
