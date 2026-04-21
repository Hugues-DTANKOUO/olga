#![allow(dead_code)]

use olga::error::Warning;
use olga::traits::{DecodeResult, FormatDecoder};
use unicode_normalization::UnicodeNormalization;

pub fn warnings_snapshot(warnings: &[Warning]) -> Vec<String> {
    warnings.iter().map(ToString::to_string).collect()
}

pub fn decode_snapshot(result: &DecodeResult) -> String {
    let metadata = serde_json::to_value(&result.metadata).unwrap();
    let primitives = serde_json::to_value(&result.primitives).unwrap();
    let warnings = serde_json::to_value(warnings_snapshot(&result.warnings)).unwrap();

    serde_json::to_string(&(metadata, primitives, warnings)).unwrap()
}

pub fn assert_contract(result: &DecodeResult) {
    for window in result.primitives.windows(2) {
        assert!(
            window[0].source_order < window[1].source_order,
            "source_order must be strictly monotonic"
        );
    }

    for primitive in &result.primitives {
        assert!(primitive.bbox.is_valid(), "invalid bbox for {}", primitive);
        if let Some(text) = primitive.text_content() {
            let normalized: String = text.nfc().collect();
            assert_eq!(text, normalized, "text must be NFC-normalized");
        }
    }
}

pub fn assert_deterministic(result_a: &DecodeResult, result_b: &DecodeResult) {
    assert_eq!(decode_snapshot(result_a), decode_snapshot(result_b));
}

pub fn assert_decode_stable(decoder: &impl FormatDecoder, data: &[u8]) -> DecodeResult {
    let first = decoder.decode(data.to_vec()).unwrap();
    let second = decoder.decode(data.to_vec()).unwrap();
    assert_deterministic(&first, &second);
    first
}
