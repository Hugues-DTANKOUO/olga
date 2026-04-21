use super::*;

#[test]
fn convert_images_filters_explicit_artifact_images() {
    let images = vec![
        PdfImage::with_bbox(
            1,
            1,
            ColorSpace::DeviceRGB,
            8,
            ImageData::Jpeg(vec![0xFF, 0xD8, 0xFF]),
            Rect::new(0.0, 0.0, 10.0, 10.0),
        ),
        PdfImage::with_bbox(
            1,
            1,
            ColorSpace::DeviceRGB,
            8,
            ImageData::Jpeg(vec![0xFF, 0xD8, 0xEE]),
            Rect::new(12.0, 12.0, 10.0, 10.0),
        ),
    ];
    let appearances = vec![
        TaggedImageAppearance {
            mcid: None,
            is_artifact: true,
            artifact_type: Some(ArtifactType::Pagination(PaginationSubtype::Watermark)),
        },
        TaggedImageAppearance {
            mcid: Some(9),
            is_artifact: false,
            artifact_type: None,
        },
    ];
    let mut warnings = Vec::new();

    let raw = convert_images(&images, None, Some(&appearances), 0, &mut warnings);

    assert_eq!(raw.len(), 1);
    assert_eq!(raw[0].data, vec![0xFF, 0xD8, 0xEE]);
    assert!(warnings.iter().any(|warning| {
        warning.kind == WarningKind::FilteredArtifact
            && warning.message.contains("pagination/watermark")
    }));
}

#[test]
fn filter_artifact_paths_filters_matching_rectangles() {
    let paths = vec![
        RawPath::Rect {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 12.0,
            fill_r: Some(255),
            fill_g: Some(255),
            fill_b: Some(255),
            stroke_r: None,
            stroke_g: None,
            stroke_b: None,
            stroke_thickness: 0.0,
        },
        RawPath::Rect {
            x: 30.0,
            y: 60.0,
            width: 40.0,
            height: 20.0,
            fill_r: None,
            fill_g: None,
            fill_b: None,
            stroke_r: Some(0),
            stroke_g: None,
            stroke_b: None,
            stroke_thickness: 1.0,
        },
    ];
    let artifacts = vec![ArtifactPathCandidate {
        path: RawPath::Rect {
            x: 10.2,
            y: 20.1,
            width: 99.7,
            height: 12.0,
            fill_r: Some(255),
            fill_g: Some(255),
            fill_b: Some(255),
            stroke_r: None,
            stroke_g: None,
            stroke_b: None,
            stroke_thickness: 0.0,
        },
        artifact_type: Some(ArtifactType::Pagination(PaginationSubtype::Watermark)),
    }];
    let mut warnings = Vec::new();

    let filtered = filter_artifact_paths(paths, Some(&artifacts), 0, &mut warnings);

    assert_eq!(filtered.len(), 1);
    assert!(matches!(filtered[0], RawPath::Rect { x, y, .. } if x == 30.0 && y == 60.0));
    assert!(warnings.iter().any(|warning| {
        warning.kind == WarningKind::FilteredArtifact
            && warning.message.contains("pagination/watermark")
    }));
}

#[test]
fn build_artifact_path_candidate_classifies_stroked_lines() {
    let builder = ArtifactPathBuilder {
        commands: vec![
            ArtifactPathCommand::MoveTo(PdfPoint { x: 12.0, y: 40.0 }),
            ArtifactPathCommand::LineTo(PdfPoint { x: 112.0, y: 40.0 }),
        ],
        has_complex_segments: false,
    };
    let mut graphics_state = pdf_oxide::content::GraphicsState::new();
    graphics_state.line_width = 2.0;
    graphics_state.stroke_color_rgb = (1.0, 0.0, 0.0);

    let path = build_artifact_path_candidate(
        &builder,
        &graphics_state,
        PaintMode {
            stroke: true,
            fill: false,
        },
    )
    .unwrap();

    assert!(matches!(
        path,
        RawPath::Line {
            x0,
            y0,
            x1,
            y1,
            thickness,
            color_r,
            color_g,
            color_b,
        } if x0 == 12.0
            && y0 == 40.0
            && x1 == 112.0
            && y1 == 40.0
            && thickness == 2.0
            && color_r == 255
            && color_g == 0
            && color_b == 0
    ));
}

#[test]
fn build_artifact_path_candidate_classifies_manual_rectangles() {
    let builder = ArtifactPathBuilder {
        commands: vec![
            ArtifactPathCommand::MoveTo(PdfPoint { x: 10.0, y: 20.0 }),
            ArtifactPathCommand::LineTo(PdfPoint { x: 60.0, y: 20.0 }),
            ArtifactPathCommand::LineTo(PdfPoint { x: 60.0, y: 50.0 }),
            ArtifactPathCommand::LineTo(PdfPoint { x: 10.0, y: 50.0 }),
            ArtifactPathCommand::ClosePath,
        ],
        has_complex_segments: false,
    };
    let mut graphics_state = pdf_oxide::content::GraphicsState::new();
    graphics_state.line_width = 1.5;
    graphics_state.stroke_color_rgb = (0.0, 0.0, 1.0);
    graphics_state.fill_color_rgb = (0.5, 0.5, 0.5);

    let path = build_artifact_path_candidate(
        &builder,
        &graphics_state,
        PaintMode {
            stroke: true,
            fill: true,
        },
    )
    .unwrap();

    assert!(matches!(
        path,
        RawPath::Rect {
            x,
            y,
            width,
            height,
            fill_r,
            fill_g,
            fill_b,
            stroke_r,
            stroke_g,
            stroke_b,
            stroke_thickness,
        } if x == 10.0
            && y == 20.0
            && width == 50.0
            && height == 30.0
            && fill_r == Some(127)
            && fill_g == Some(127)
            && fill_b == Some(127)
            && stroke_r == Some(0)
            && stroke_g == Some(0)
            && stroke_b == Some(255)
            && stroke_thickness == 1.5
    ));
}

#[test]
fn finalize_artifact_path_candidate_tracks_unclassified_complex_artifacts() {
    let mut warnings = Vec::new();
    let mut state = PathArtifactScanState::new(0, &mut warnings);
    state.marked_content_stack.push(MarkedContentContext {
        mcid: None,
        is_artifact: true,
        artifact_type: Some(ArtifactType::Background),
    });
    state.current_path = ArtifactPathBuilder {
        commands: vec![ArtifactPathCommand::MoveTo(PdfPoint { x: 0.0, y: 0.0 })],
        has_complex_segments: true,
    };

    finalize_artifact_path_candidate(
        &mut state,
        PaintMode {
            stroke: true,
            fill: false,
        },
    );

    assert!(state.artifact_paths.is_empty());
    assert_eq!(
        state.unsupported_artifact_counts.get("background"),
        Some(&1usize)
    );
}
