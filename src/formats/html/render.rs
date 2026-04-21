use scraper::ElementRef;

use crate::error::{Warning, WarningKind};
use crate::model::{TextDirection, ValueOrigin};

use super::types::{HtmlRenderContext, HtmlWhitespaceMode, WalkState};

pub(crate) fn derive_render_context(
    element: ElementRef,
    parent: &HtmlRenderContext,
    state: &mut WalkState,
    warnings: &mut Vec<Warning>,
) -> Option<HtmlRenderContext> {
    if is_hidden_element(element) {
        return None;
    }

    let mut context = HtmlRenderContext {
        lang: parent.lang.clone(),
        lang_origin: parent.lang.as_ref().map(|_| ValueOrigin::Reconstructed),
        text_direction: parent.text_direction,
        text_direction_origin: inherited_direction_origin(parent),
        auto_direction: parent.auto_direction,
        whitespace_mode: derive_whitespace_mode(element, parent.whitespace_mode),
        list_ordered_hint: parent.list_ordered_hint,
    };

    if let Some(lang) = element
        .value()
        .attr("lang")
        .filter(|lang| !lang.trim().is_empty())
    {
        context.lang = Some(lang.trim().to_string());
        context.lang_origin = Some(ValueOrigin::Observed);
    }

    let mut saw_direction_override = false;
    if let Some(dir) = element.value().attr("dir") {
        match dir.trim().to_ascii_lowercase().as_str() {
            "rtl" => {
                context.text_direction = TextDirection::RightToLeft;
                context.text_direction_origin = ValueOrigin::Observed;
                context.auto_direction = false;
                saw_direction_override = true;
            }
            "ltr" => {
                context.text_direction = TextDirection::LeftToRight;
                context.text_direction_origin = ValueOrigin::Observed;
                context.auto_direction = false;
                saw_direction_override = true;
            }
            "auto" => {
                context.auto_direction = true;
                context.text_direction_origin = ValueOrigin::Estimated;
                saw_direction_override = true;
            }
            other => report_render_limitation(
                state,
                warnings,
                format!("dir:{other}"),
                format!(
                    "HTML dir='{}' is not supported by the rendered subset and was ignored",
                    other
                ),
            ),
        }
    }

    for_each_style_declaration(
        element.value().attr("style"),
        |property, value| match property {
            "direction" => match value {
                "rtl" => {
                    context.text_direction = TextDirection::RightToLeft;
                    context.text_direction_origin = ValueOrigin::Observed;
                    context.auto_direction = false;
                    saw_direction_override = true;
                }
                "ltr" => {
                    context.text_direction = TextDirection::LeftToRight;
                    context.text_direction_origin = ValueOrigin::Observed;
                    context.auto_direction = false;
                    saw_direction_override = true;
                }
                other => report_render_limitation(
                    state,
                    warnings,
                    format!("direction:{other}"),
                    format!(
                        "HTML CSS direction:{} is outside the supported rendered subset and was ignored",
                        other
                    ),
                ),
            },
            "writing-mode" => match value {
                "vertical-rl" | "vertical-lr" | "tb" | "tb-rl" => {
                    context.text_direction = TextDirection::TopToBottom;
                    context.text_direction_origin = ValueOrigin::Observed;
                    context.auto_direction = false;
                    saw_direction_override = true;
                }
                "horizontal-tb" => {
                    context.text_direction = TextDirection::LeftToRight;
                    context.text_direction_origin = ValueOrigin::Observed;
                    context.auto_direction = false;
                    saw_direction_override = true;
                }
                "sideways-rl" | "sideways-lr" => report_render_limitation(
                    state,
                    warnings,
                    format!("writing-mode:{value}"),
                    format!(
                        "HTML CSS writing-mode:{} is not supported by the rendered subset and was ignored",
                        value
                    ),
                ),
                _ => {}
            },
            "display"
                if matches!(
                    value,
                    "flex" | "inline-flex" | "grid" | "inline-grid" | "contents"
                ) =>
            {
                report_render_limitation(
                    state,
                    warnings,
                    format!("display:{value}"),
                    format!(
                        "HTML CSS display:{} is outside the supported rendered subset; content order is preserved but layout is not",
                        value
                    ),
                );
            }
            "position" if value != "static" => {
                report_render_limitation(
                    state,
                    warnings,
                    format!("position:{value}"),
                    format!(
                        "HTML CSS position:{} is outside the supported rendered subset; geometry remains DOM-flow synthetic",
                        value
                    ),
                );
            }
            "float" if value != "none" => {
                report_render_limitation(
                    state,
                    warnings,
                    format!("float:{value}"),
                    format!(
                        "HTML CSS float:{} is outside the supported rendered subset; geometry remains DOM-flow synthetic",
                        value
                    ),
                );
            }
            "transform" if value != "none" => {
                report_render_limitation(
                    state,
                    warnings,
                    format!("transform:{value}"),
                    "HTML CSS transform is outside the supported rendered subset; geometry remains DOM-flow synthetic"
                        .to_string(),
                );
            }
            "column-count" | "column-width" | "columns" => {
                report_render_limitation(
                    state,
                    warnings,
                    format!("{property}:{value}"),
                    format!(
                        "HTML multicolumn CSS ({}) is outside the supported rendered subset; content order is preserved but layout is not",
                        property
                    ),
                );
            }
            "list-style-type" => {
                if let Some(ordered) = classify_list_style_type(value) {
                    context.list_ordered_hint = Some(ordered);
                } else {
                    report_render_limitation(
                        state,
                        warnings,
                        format!("list-style-type:{value}"),
                        format!(
                            "HTML CSS list-style-type:{} is outside the supported rendered subset and was ignored for list ordering",
                            value
                        ),
                    );
                }
            }
            _ => {}
        },
    );

    if !saw_direction_override
        && context.auto_direction
        && context.text_direction_origin == ValueOrigin::Synthetic
    {
        context.text_direction_origin = ValueOrigin::Estimated;
    }

    Some(context)
}

pub(crate) fn derive_whitespace_mode(
    element: ElementRef,
    inherited: HtmlWhitespaceMode,
) -> HtmlWhitespaceMode {
    if element.value().name() == "pre" {
        return HtmlWhitespaceMode::Preserve;
    }

    let mut mode = inherited;
    for_each_style_declaration(element.value().attr("style"), |property, value| {
        if property == "white-space" {
            mode = match value {
                "pre" | "pre-wrap" | "break-spaces" => HtmlWhitespaceMode::Preserve,
                "pre-line" => HtmlWhitespaceMode::PreserveLineBreaks,
                "normal" | "nowrap" => HtmlWhitespaceMode::Collapse,
                _ => mode,
            };
        }
    });
    mode
}

pub(crate) fn report_render_limitation(
    state: &mut WalkState,
    warnings: &mut Vec<Warning>,
    key: String,
    message: String,
) {
    if state.reported_render_limitations.insert(key) {
        warnings.push(Warning {
            kind: WarningKind::UnsupportedElement,
            message,
            page: Some(0),
        });
    }
}

pub(crate) fn report_heuristic_inference(
    state: &mut WalkState,
    warnings: &mut Vec<Warning>,
    key: String,
    message: String,
) {
    if state.reported_heuristic_inferences.insert(key) {
        warnings.push(Warning {
            kind: WarningKind::HeuristicInference,
            message,
            page: Some(0),
        });
    }
}

/// Decide whether an element is visually hidden and should be skipped.
///
/// We use the same contract as mainstream document extractors — pandoc,
/// Apache Tika, Readability, html2text — which respect *visual* hiding
/// signals but NOT accessibility-tree hints.
///
/// Specifically:
///
/// * `hidden` HTML attribute → hides the element entirely (W3C HTML § 3.2.6)
/// * inline `display:none` → element is not rendered
/// * inline `visibility:hidden`/`collapse` → box is reserved but content is
///   invisible; extractors conventionally skip it
/// * inline `content-visibility:hidden` → same semantics as display:none
///   for rendering
///
/// `aria-hidden="true"` is intentionally NOT treated as a visibility signal.
/// Per the WAI-ARIA spec (§ 6.8), aria-hidden removes an element from the
/// accessibility tree — screen readers skip it — but has NO effect on visual
/// rendering. It is commonly used on decorative icons that duplicate nearby
/// text, but it is ALSO used on visible content that authors don't want
/// announced (e.g. FAQ internal notes, visible debugging markers, the
/// "aria-hidden" badge on the initial DOM element of React trees during
/// transitions). Treating it as a hide signal causes us to drop authored
/// content that every other mainstream extractor surfaces.
///
/// Stylesheet-based `display:none` (via `<style>` or external CSS) is not
/// detected because we don't evaluate stylesheets; authors relying on CSS
/// class hiding must additionally set the `hidden` attribute or an inline
/// style to guarantee extraction behaviour. This matches pandoc / Tika.
pub(crate) fn is_hidden_element(element: ElementRef) -> bool {
    if element.value().attr("hidden").is_some() {
        return true;
    }

    let mut hidden = false;
    for_each_style_declaration(
        element.value().attr("style"),
        |property, value| match property {
            "display" if value == "none" => hidden = true,
            "visibility" if matches!(value, "hidden" | "collapse") => hidden = true,
            "content-visibility" if value == "hidden" => hidden = true,
            _ => {}
        },
    );
    hidden
}

pub(crate) fn for_each_style_declaration(style: Option<&str>, mut f: impl FnMut(&str, &str)) {
    let Some(style) = style else {
        return;
    };

    for declaration in style.split(';') {
        let Some((property, value)) = declaration.split_once(':') else {
            continue;
        };
        let property = property.trim().to_ascii_lowercase();
        let value = value.trim().to_ascii_lowercase();
        if property.is_empty() || value.is_empty() {
            continue;
        }
        f(property.as_str(), value.as_str());
    }
}

pub(crate) fn classify_list_style_type(value: &str) -> Option<bool> {
    match value {
        "disc" | "circle" | "square" | "none" | "disclosure-open" | "disclosure-closed" => {
            Some(false)
        }
        "decimal"
        | "decimal-leading-zero"
        | "lower-roman"
        | "upper-roman"
        | "lower-alpha"
        | "upper-alpha"
        | "lower-latin"
        | "upper-latin"
        | "lower-greek"
        | "armenian"
        | "georgian" => Some(true),
        _ => None,
    }
}

fn inherited_direction_origin(parent: &HtmlRenderContext) -> ValueOrigin {
    match parent.text_direction_origin {
        ValueOrigin::Observed => ValueOrigin::Reconstructed,
        other => other,
    }
}
