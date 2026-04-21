use super::styles::resolve_style;
use super::types::{RelType, ResolvedStyle};
use crate::formats::xml_utils::*;
use crate::model::ImageFormat;
use std::collections::HashMap;

#[test]
fn local_name_with_prefix() {
    assert_eq!(local_name(b"w:p"), b"p");
    assert_eq!(local_name(b"w:tbl"), b"tbl");
    assert_eq!(local_name(b"a:blip"), b"blip");
}

#[test]
fn local_name_without_prefix() {
    assert_eq!(local_name(b"Relationship"), b"Relationship");
}

#[test]
fn rel_type_parsing() {
    assert_eq!(
        RelType::from_uri(
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image"
        ),
        RelType::Image
    );
    assert_eq!(
        RelType::from_uri(
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink"
        ),
        RelType::Hyperlink
    );
    assert_eq!(
        RelType::from_uri(
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header"
        ),
        RelType::Header
    );
}

#[test]
fn parse_hex_color_valid() {
    let c = parse_hex_color("FF0000").unwrap();
    assert_eq!(c.r, 255);
    assert_eq!(c.g, 0);
    assert_eq!(c.b, 0);
}

#[test]
fn parse_hex_color_auto() {
    assert!(parse_hex_color("auto").is_none());
}

#[test]
fn parse_hex_color_with_hash() {
    let c = parse_hex_color("#00FF00").unwrap();
    assert_eq!(c.g, 255);
}

#[test]
fn guess_image_format_png() {
    assert_eq!(
        guess_image_format(&[0x89, 0x50, 0x4E, 0x47]),
        ImageFormat::Png
    );
}

#[test]
fn guess_image_format_jpeg() {
    assert_eq!(
        guess_image_format(&[0xFF, 0xD8, 0xFF, 0xE0]),
        ImageFormat::Jpeg
    );
}

#[test]
fn guess_image_format_unknown() {
    assert_eq!(
        guess_image_format(&[0x00, 0x01, 0x02]),
        ImageFormat::Unknown
    );
}

#[test]
fn style_resolution_with_inheritance() {
    let mut raw = HashMap::new();
    raw.insert(
        "Normal".to_string(),
        ResolvedStyle {
            style_id: "Normal".to_string(),
            name: "Normal".to_string(),
            font_size_half_pt: Some(22),
            font_name: Some("Calibri".to_string()),
            is_bold: Some(false),
            ..Default::default()
        },
    );
    raw.insert(
        "Heading1".to_string(),
        ResolvedStyle {
            style_id: "Heading1".to_string(),
            name: "Heading 1".to_string(),
            font_size_half_pt: Some(32),
            is_bold: Some(true),
            outline_level: Some(0),
            based_on: Some("Normal".to_string()),
            ..Default::default()
        },
    );
    raw.insert(
        "Heading2".to_string(),
        ResolvedStyle {
            style_id: "Heading2".to_string(),
            name: "Heading 2".to_string(),
            font_size_half_pt: Some(26),
            outline_level: Some(1),
            based_on: Some("Heading1".to_string()),
            ..Default::default()
        },
    );

    let h2 = resolve_style("Heading2", &raw, 0);
    assert_eq!(h2.font_size_half_pt, Some(26));
    assert_eq!(h2.is_bold, Some(true));
    assert_eq!(h2.font_name, Some("Calibri".to_string()));
    assert_eq!(h2.outline_level, Some(1));
}

#[test]
fn is_w_element_checks() {
    assert!(is_w_element(b"w:p"));
    assert!(is_w_element(b"p"));
    assert!(!is_w_element(b"a:blip"));
}

#[test]
fn style_circular_reference_returns_default() {
    let mut raw = HashMap::new();
    raw.insert(
        "A".to_string(),
        ResolvedStyle {
            style_id: "A".to_string(),
            name: "Style A".to_string(),
            based_on: Some("B".to_string()),
            font_size_half_pt: Some(20),
            ..Default::default()
        },
    );
    raw.insert(
        "B".to_string(),
        ResolvedStyle {
            style_id: "B".to_string(),
            name: "Style B".to_string(),
            based_on: Some("A".to_string()),
            ..Default::default()
        },
    );
    let result = resolve_style("A", &raw, 0);
    assert_eq!(result.font_size_half_pt, Some(20));
}

#[test]
fn style_missing_parent_returns_own_values() {
    let mut raw = HashMap::new();
    raw.insert(
        "Child".to_string(),
        ResolvedStyle {
            style_id: "Child".to_string(),
            name: "Child Style".to_string(),
            based_on: Some("NonExistent".to_string()),
            is_bold: Some(true),
            ..Default::default()
        },
    );
    let result = resolve_style("Child", &raw, 0);
    assert_eq!(result.is_bold, Some(true));
}
