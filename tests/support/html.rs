#![allow(dead_code)]

use olga::formats::html::HtmlDecoder;
use olga::traits::{DecodeResult, FormatDecoder};

pub fn decode_html(html: &str) -> DecodeResult {
    let decoder = HtmlDecoder::new();
    decoder
        .decode(html.as_bytes().to_vec())
        .expect("decode should succeed")
}

pub fn text_primitives(result: &DecodeResult) -> Vec<&str> {
    result
        .primitives
        .iter()
        .filter_map(|p| p.text_content())
        .filter(|t| !t.is_empty())
        .collect()
}
