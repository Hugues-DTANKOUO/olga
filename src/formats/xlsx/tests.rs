use super::*;
use crate::traits::FormatDecoder;

#[test]
fn decoder_name_and_extensions() {
    let decoder = XlsxDecoder::new();
    assert_eq!(decoder.name(), "XLSX");
    assert!(decoder.supported_extensions().contains(&"xlsx"));
    assert!(decoder.supported_extensions().contains(&"xlsm"));
}

#[test]
fn decode_invalid_data_returns_error() {
    let decoder = XlsxDecoder::new();
    let result = decoder.metadata(b"not a zip file");
    assert!(result.is_err());
}
