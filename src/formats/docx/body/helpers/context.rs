use std::collections::HashMap;

use quick_xml::events::BytesStart;

use crate::formats::xml_utils::{attr_value, bool_attr, local_name, parse_hex_color};
use crate::model::{Primitive, SemanticHint, TextDirection};

use super::super::super::types::{ParagraphAlignment, ParagraphContext, ResolvedStyle, RunContext};
use super::super::tables::ActiveTableState;
use super::super::tables::VerticalMergeRole;

pub(crate) fn story_id_from_element(e: &BytesStart<'_>) -> Option<String> {
    e.attributes()
        .flatten()
        .find_map(|attr| (local_name(attr.key.as_ref()) == b"id").then(|| attr_value(&attr)))
}

pub(crate) fn push_hint_once(prim: &mut Primitive, hint: SemanticHint) {
    if !prim.hints.contains(&hint) {
        prim.hints.push(hint);
    }
}

pub(crate) fn start_docx_paragraph_context(
    current_story_id: Option<&str>,
    in_textbox: bool,
) -> ParagraphContext {
    ParagraphContext {
        story_id: current_story_id.map(str::to_string),
        in_textbox,
        ..Default::default()
    }
}

pub(crate) fn start_docx_run_context(
    para: &ParagraphContext,
    styles: &HashMap<String, ResolvedStyle>,
    hyperlink_url: Option<&str>,
) -> RunContext {
    let style = para.style_id.as_ref().and_then(|id| styles.get(id));

    RunContext {
        font_size: style
            .and_then(|s| s.font_size_half_pt)
            .map(|hp| hp as f32 / 2.0)
            .unwrap_or(11.0),
        font_name: style
            .and_then(|s| s.font_name.clone())
            .unwrap_or_else(|| "Calibri".to_string()),
        is_bold: style.and_then(|s| s.is_bold).unwrap_or(false),
        is_italic: style.and_then(|s| s.is_italic).unwrap_or(false),
        color: style
            .and_then(|s| s.color.as_ref())
            .and_then(|c| parse_hex_color(c)),
        text_direction: if para.is_bidi {
            TextDirection::RightToLeft
        } else {
            TextDirection::LeftToRight
        },
        hyperlink_url: hyperlink_url.map(str::to_string),
        ..Default::default()
    }
}

pub(crate) fn apply_docx_table_property(
    table_stack: &mut [ActiveTableState],
    name: &[u8],
    e: &BytesStart<'_>,
) {
    match name {
        b"gridSpan" => {
            for attr in e.attributes().flatten() {
                if local_name(attr.key.as_ref()) == b"val"
                    && let Ok(span) = attr_value(&attr).parse::<u32>()
                    && let Some(table) = table_stack.last_mut()
                {
                    table.set_current_colspan(span);
                }
            }
        }
        b"vMerge" => {
            if let Some(table) = table_stack.last_mut() {
                table.set_vertical_merge_role(parse_docx_vmerge_role(e));
            }
        }
        _ => {}
    }
}

pub(crate) fn apply_docx_paragraph_property(
    para: &mut ParagraphContext,
    name: &[u8],
    e: &BytesStart<'_>,
) {
    match name {
        b"pStyle" => {
            for attr in e.attributes().flatten() {
                if local_name(attr.key.as_ref()) == b"val" {
                    para.style_id = Some(attr_value(&attr));
                }
            }
        }
        b"numId" => {
            for attr in e.attributes().flatten() {
                if local_name(attr.key.as_ref()) == b"val" {
                    let v = attr_value(&attr);
                    if v != "0" {
                        para.num_id = Some(v);
                    }
                }
            }
        }
        b"ilvl" => {
            for attr in e.attributes().flatten() {
                if local_name(attr.key.as_ref()) == b"val" {
                    para.num_level = attr_value(&attr).parse().ok();
                }
            }
        }
        b"bidi" => {
            para.is_bidi = true;
        }
        b"jc" => {
            for attr in e.attributes().flatten() {
                if local_name(attr.key.as_ref()) == b"val" {
                    para.alignment = match attr_value(&attr).as_str() {
                        "center" => ParagraphAlignment::Center,
                        "right" | "end" => ParagraphAlignment::Right,
                        "both" | "distribute" => ParagraphAlignment::Both,
                        _ => ParagraphAlignment::Left,
                    };
                }
            }
        }
        _ => {}
    }
}

pub(crate) fn apply_docx_run_property(run: &mut RunContext, name: &[u8], e: &BytesStart<'_>) {
    match name {
        b"b" => run.is_bold = bool_attr(e, true),
        b"i" => run.is_italic = bool_attr(e, true),
        b"sz" => {
            for attr in e.attributes().flatten() {
                if local_name(attr.key.as_ref()) == b"val"
                    && let Ok(v) = attr_value(&attr).parse::<u32>()
                {
                    let pt = v as f32 / 2.0;
                    run.font_size = if pt > 0.0 { pt } else { 11.0 };
                }
            }
        }
        b"rFonts" => {
            for attr in e.attributes().flatten() {
                let k = local_name(attr.key.as_ref());
                if k == b"ascii" || k == b"hAnsi" || k == b"cs" {
                    run.font_name = attr_value(&attr);
                    break;
                }
            }
        }
        b"color" => {
            for attr in e.attributes().flatten() {
                if local_name(attr.key.as_ref()) == b"val" {
                    run.color = parse_hex_color(&attr_value(&attr));
                }
            }
        }
        b"rtl" => {
            run.text_direction = TextDirection::RightToLeft;
        }
        b"lang" => {
            let mut picked: Option<String> = None;
            for attr in e.attributes().flatten() {
                let aname = local_name(attr.key.as_ref());
                let val = attr_value(&attr);
                match aname.as_slice() {
                    b"val" => {
                        picked = Some(val);
                        break;
                    }
                    b"bidi" if picked.is_none() => {
                        picked = Some(val);
                    }
                    b"eastAsia" if picked.is_none() => {
                        picked = Some(val);
                    }
                    _ => {}
                }
            }
            if let Some(lang) = picked {
                run.lang = Some(lang);
            }
        }
        _ => {}
    }
}

pub(crate) fn parse_docx_vmerge_role(e: &BytesStart<'_>) -> VerticalMergeRole {
    for attr in e.attributes().flatten() {
        if local_name(attr.key.as_ref()) == b"val" {
            return match attr_value(&attr).as_str() {
                "restart" => VerticalMergeRole::Restart,
                "continue" => VerticalMergeRole::Continue,
                _ => VerticalMergeRole::Continue,
            };
        }
    }
    VerticalMergeRole::Continue
}
