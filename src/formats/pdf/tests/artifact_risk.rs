use super::*;

#[test]
fn repeated_vector_artifact_signature_detects_top_band_lines() {
    let page_dims = PageDimensions::new(RawBox::new(0.0, 0.0, 100.0, 100.0), None, 0);
    let path = RawPath::Line {
        x0: 5.0,
        y0: 95.0,
        x1: 95.0,
        y1: 95.0,
        thickness: 1.0,
        color_r: 0,
        color_g: 0,
        color_b: 0,
    };

    let signature = repeated_vector_artifact_signature(&path, &page_dims).unwrap();

    assert_eq!(signature.kind, "line");
    assert_eq!(signature.band, ArtifactBand::Top);
}

#[test]
fn repeated_vector_artifact_signature_ignores_mid_page_shapes() {
    let page_dims = PageDimensions::new(RawBox::new(0.0, 0.0, 100.0, 100.0), None, 0);
    let path = RawPath::Rect {
        x: 20.0,
        y: 40.0,
        width: 60.0,
        height: 8.0,
        fill_r: Some(200),
        fill_g: Some(200),
        fill_b: Some(200),
        stroke_r: None,
        stroke_g: None,
        stroke_b: None,
        stroke_thickness: 0.0,
    };

    assert!(repeated_vector_artifact_signature(&path, &page_dims).is_none());
}

#[test]
fn emit_repeated_vector_artifact_risk_warnings_requires_repetition() {
    let mut warnings = Vec::new();
    let candidate = RepeatedVectorArtifactCandidate {
        page: 0,
        band: ArtifactBand::Top,
        signature: RepeatedVectorArtifactSignature {
            kind: "line",
            band: ArtifactBand::Top,
            qx: 10,
            qy: 10,
            qw: 800,
            qh: 0,
            style: 0,
        },
    };

    emit_repeated_vector_artifact_risk_warnings(&[candidate], &mut warnings);

    assert!(warnings.is_empty());
}

#[test]
fn emit_repeated_vector_artifact_risk_warnings_marks_repeated_patterns() {
    let mut warnings = Vec::new();
    let signature = RepeatedVectorArtifactSignature {
        kind: "rect",
        band: ArtifactBand::Bottom,
        qx: 0,
        qy: 920,
        qw: 1000,
        qh: 40,
        style: 123,
    };
    let candidates = vec![
        RepeatedVectorArtifactCandidate {
            page: 0,
            band: ArtifactBand::Bottom,
            signature: signature.clone(),
        },
        RepeatedVectorArtifactCandidate {
            page: 1,
            band: ArtifactBand::Bottom,
            signature,
        },
    ];

    emit_repeated_vector_artifact_risk_warnings(&candidates, &mut warnings);

    assert_eq!(warnings.len(), 2);
    assert!(
        warnings
            .iter()
            .all(|warning| warning.kind == WarningKind::SuspectedArtifact)
    );
    assert!(
        warnings
            .iter()
            .all(|warning| warning.message.contains("untagged PDF rect pattern"))
    );
}

#[test]
fn repeated_image_artifact_signature_detects_top_band_images() {
    let page_dims = PageDimensions::new(RawBox::new(0.0, 0.0, 100.0, 100.0), None, 0);
    let image = RawImage {
        data: vec![1, 2, 3, 4],
        format: crate::model::ImageFormat::Png,
        alt_text: None,
        mcid: None,
        raw_x: 10.0,
        raw_y: 88.0,
        raw_width: 20.0,
        raw_height: 10.0,
    };

    let signature = repeated_image_artifact_signature(&image, &page_dims).unwrap();

    assert_eq!(signature.band, ArtifactBand::Top);
    assert_eq!(signature.format, crate::model::ImageFormat::Png);
}

#[test]
fn repeated_image_artifact_signature_ignores_small_mid_page_images() {
    let page_dims = PageDimensions::new(RawBox::new(0.0, 0.0, 100.0, 100.0), None, 0);
    let image = RawImage {
        data: vec![1, 2, 3, 4],
        format: crate::model::ImageFormat::Png,
        alt_text: None,
        mcid: None,
        raw_x: 45.0,
        raw_y: 45.0,
        raw_width: 4.0,
        raw_height: 4.0,
    };

    assert!(repeated_image_artifact_signature(&image, &page_dims).is_none());
}

#[test]
fn emit_repeated_image_artifact_risk_warnings_requires_repetition() {
    let mut warnings = Vec::new();
    let candidate = RepeatedImageArtifactCandidate {
        page: 0,
        band: ArtifactBand::Top,
        signature: RepeatedImageArtifactSignature {
            band: ArtifactBand::Top,
            qx: 100,
            qy: 20,
            qw: 200,
            qh: 100,
            format: crate::model::ImageFormat::Png,
            data_fingerprint: 42,
        },
    };

    emit_repeated_image_artifact_risk_warnings(&[candidate], &mut warnings);

    assert!(warnings.is_empty());
}

#[test]
fn emit_repeated_image_artifact_risk_warnings_marks_repeated_patterns() {
    let mut warnings = Vec::new();
    let signature = RepeatedImageArtifactSignature {
        band: ArtifactBand::Bottom,
        qx: 0,
        qy: 900,
        qw: 120,
        qh: 80,
        format: crate::model::ImageFormat::Jpeg,
        data_fingerprint: 777,
    };
    let candidates = vec![
        RepeatedImageArtifactCandidate {
            page: 0,
            band: ArtifactBand::Bottom,
            signature: signature.clone(),
        },
        RepeatedImageArtifactCandidate {
            page: 1,
            band: ArtifactBand::Bottom,
            signature,
        },
    ];

    emit_repeated_image_artifact_risk_warnings(&candidates, &mut warnings);

    assert_eq!(warnings.len(), 2);
    assert!(
        warnings
            .iter()
            .all(|warning| warning.kind == WarningKind::SuspectedArtifact)
    );
    assert!(
        warnings
            .iter()
            .all(|warning| warning.message.contains("untagged PDF image pattern"))
    );
}
