use pdf_oxide::PdfDocument;
use pdf_oxide::content::{Matrix, Operator, parse_content_stream};
use pdf_oxide::object::{Object, ObjectRef};

use super::super::marked_content::{
    active_marked_content_artifact, build_marked_content_context, marked_content_context_for_tag,
};
use super::super::resources::{load_page_resources, resolve_named_xobject, resolve_object};
use super::classify::build_artifact_path_candidate;
use super::geometry::{extract_form_xobject_matrix, transform_pdf_point, transform_pdf_rect};
use super::*;

pub(super) fn scan_page_artifact_paths(
    doc: &mut PdfDocument,
    page_idx: usize,
    warnings: &mut Vec<Warning>,
    cached_resources: Option<&Object>,
) -> Option<Vec<ArtifactPathCandidate>> {
    let page_num = page_idx as u32;
    let owned_resources;
    let resources = match cached_resources {
        Some(r) => r.clone(),
        None => {
            owned_resources = match load_page_resources(doc, page_idx) {
                Ok(resources) => resources,
                Err(err) => {
                    warnings.push(Warning {
                        kind: WarningKind::PartialExtraction,
                        message: format!(
                            "Failed to resolve page resources while scanning PDF vector artifacts: {}",
                            err
                        ),
                        page: Some(page_num),
                    });
                    return None;
                }
            };
            owned_resources
        }
    };

    let content_data = match doc.get_page_content_data(page_idx) {
        Ok(content_data) => content_data,
        Err(err) => {
            warnings.push(Warning {
                kind: WarningKind::PartialExtraction,
                message: format!(
                    "Failed to read page content while scanning PDF vector artifacts: {}",
                    err
                ),
                page: Some(page_num),
            });
            return None;
        }
    };

    let operators = match parse_content_stream(&content_data) {
        Ok(operators) => operators,
        Err(err) => {
            warnings.push(Warning {
                kind: WarningKind::PartialExtraction,
                message: format!(
                    "Failed to parse page content while scanning PDF vector artifacts: {}",
                    err
                ),
                page: Some(page_num),
            });
            return None;
        }
    };

    let mut state = PathArtifactScanState::new(page_num, warnings);
    collect_artifact_paths_from_operators(doc, &operators, &resources, &mut state);
    if !state.unsupported_artifact_counts.is_empty() {
        let total = state
            .unsupported_artifact_counts
            .values()
            .copied()
            .sum::<usize>();
        let details = state
            .unsupported_artifact_counts
            .iter()
            .map(|(label, count)| format!("{} {}", count, label))
            .collect::<Vec<_>>()
            .join(", ");
        state.warn(format!(
            "Detected {} explicit PDF vector artifact path(s) too complex to correlate precisely; leaving them unfiltered ({})",
            total, details
        ));
    }
    Some(state.artifact_paths)
}

pub(in crate::formats::pdf) fn finalize_artifact_path_candidate(
    state: &mut PathArtifactScanState<'_>,
    paint_mode: PaintMode,
) {
    let active_artifact = active_marked_content_artifact(&state.marked_content_stack);
    let artifact_type = active_artifact.and_then(|ctx| ctx.artifact_type.clone());

    if artifact_type.is_some() || active_artifact.is_some() {
        if let Some(path) = build_artifact_path_candidate(
            &state.current_path,
            state.graphics_state_stack.current(),
            paint_mode,
        ) {
            state.artifact_paths.push(ArtifactPathCandidate {
                path,
                artifact_type,
            });
        } else if state.current_path.has_path() {
            *state
                .unsupported_artifact_counts
                .entry(describe_artifact_kind(artifact_type.as_ref()))
                .or_insert(0usize) += 1;
        }
    }

    state.current_path.clear();
}

fn collect_artifact_paths_from_operators(
    doc: &mut PdfDocument,
    operators: &[Operator],
    resources: &Object,
    state: &mut PathArtifactScanState<'_>,
) {
    for operator in operators {
        match operator {
            Operator::SaveState => {
                state.graphics_state_stack.save();
            }
            Operator::RestoreState => {
                state.graphics_state_stack.restore();
            }
            Operator::Cm { a, b, c, d, e, f } => {
                let graphics_state = state.graphics_state_stack.current_mut();
                let new_matrix = Matrix {
                    a: *a,
                    b: *b,
                    c: *c,
                    d: *d,
                    e: *e,
                    f: *f,
                };
                graphics_state.ctm = new_matrix.multiply(&graphics_state.ctm);
            }
            Operator::SetStrokeRgb { r, g, b } => {
                state.graphics_state_stack.current_mut().stroke_color_rgb = (*r, *g, *b);
            }
            Operator::SetStrokeGray { gray } => {
                state.graphics_state_stack.current_mut().stroke_color_rgb = (*gray, *gray, *gray);
            }
            Operator::SetStrokeCmyk { c, m, y, k } => {
                state.graphics_state_stack.current_mut().stroke_color_rgb = (
                    (1.0 - *c) * (1.0 - *k),
                    (1.0 - *m) * (1.0 - *k),
                    (1.0 - *y) * (1.0 - *k),
                );
            }
            Operator::SetFillRgb { r, g, b } => {
                state.graphics_state_stack.current_mut().fill_color_rgb = (*r, *g, *b);
            }
            Operator::SetFillGray { gray } => {
                state.graphics_state_stack.current_mut().fill_color_rgb = (*gray, *gray, *gray);
            }
            Operator::SetFillCmyk { c, m, y, k } => {
                state.graphics_state_stack.current_mut().fill_color_rgb = (
                    (1.0 - *c) * (1.0 - *k),
                    (1.0 - *m) * (1.0 - *k),
                    (1.0 - *y) * (1.0 - *k),
                );
            }
            Operator::SetLineWidth { width } => {
                state.graphics_state_stack.current_mut().line_width = *width;
            }
            Operator::BeginMarkedContent { tag } => {
                state
                    .marked_content_stack
                    .push(marked_content_context_for_tag(tag));
            }
            Operator::BeginMarkedContentDict { tag, properties } => {
                state
                    .marked_content_stack
                    .push(build_marked_content_context(
                        doc, tag, properties, resources,
                    ));
            }
            Operator::EndMarkedContent => {
                let _ = state.marked_content_stack.pop();
            }
            Operator::MoveTo { x, y } => {
                state
                    .current_path
                    .commands
                    .push(ArtifactPathCommand::MoveTo(transform_pdf_point(
                        state.graphics_state_stack.current().ctm,
                        *x,
                        *y,
                    )));
            }
            Operator::LineTo { x, y } => {
                state
                    .current_path
                    .commands
                    .push(ArtifactPathCommand::LineTo(transform_pdf_point(
                        state.graphics_state_stack.current().ctm,
                        *x,
                        *y,
                    )));
            }
            Operator::Rectangle {
                x,
                y,
                width,
                height,
            } => {
                state
                    .current_path
                    .commands
                    .push(ArtifactPathCommand::Rectangle(transform_pdf_rect(
                        state.graphics_state_stack.current().ctm,
                        *x,
                        *y,
                        *width,
                        *height,
                    )));
            }
            Operator::ClosePath => {
                state
                    .current_path
                    .commands
                    .push(ArtifactPathCommand::ClosePath);
            }
            Operator::CurveTo { .. } | Operator::CurveToV { .. } | Operator::CurveToY { .. } => {
                state.current_path.has_complex_segments = true;
            }
            Operator::Stroke => finalize_artifact_path_candidate(
                state,
                PaintMode {
                    stroke: true,
                    fill: false,
                },
            ),
            Operator::Fill | Operator::FillEvenOdd => finalize_artifact_path_candidate(
                state,
                PaintMode {
                    stroke: false,
                    fill: true,
                },
            ),
            Operator::FillStroke
            | Operator::FillStrokeEvenOdd
            | Operator::CloseFillStroke
            | Operator::CloseFillStrokeEvenOdd => finalize_artifact_path_candidate(
                state,
                PaintMode {
                    stroke: true,
                    fill: true,
                },
            ),
            Operator::EndPath => {
                state.current_path.clear();
            }
            Operator::Do { name } => {
                let Some((xobject, xobject_ref)) =
                    resolve_named_xobject(doc, resources, name.as_str())
                else {
                    continue;
                };

                if state.current_path.has_path() {
                    state.current_path.clear();
                }

                if matches!(
                    xobject
                        .as_dict()
                        .and_then(|dict| dict.get("Subtype"))
                        .and_then(Object::as_name),
                    Some("Form")
                ) {
                    scan_form_xobject_artifact_paths(doc, &xobject, xobject_ref, resources, state);
                }
            }
            _ => {}
        }
    }
}

fn scan_form_xobject_artifact_paths(
    doc: &mut PdfDocument,
    xobject: &Object,
    xobject_ref: Option<ObjectRef>,
    parent_resources: &Object,
    state: &mut PathArtifactScanState<'_>,
) {
    if let Some(xobject_ref) = xobject_ref {
        if state.xobject_stack.contains(&xobject_ref) {
            return;
        }
        state.xobject_stack.push(xobject_ref);
    }

    let Some(xobject_dict) = xobject.as_dict() else {
        if xobject_ref.is_some() {
            let _ = state.xobject_stack.pop();
        }
        return;
    };

    let form_resources = xobject_dict
        .get("Resources")
        .and_then(|resources| resolve_object(doc, resources))
        .unwrap_or_else(|| parent_resources.clone());
    let form_matrix = extract_form_xobject_matrix(xobject_dict);

    let stream_data = match xobject.decode_stream_data() {
        Ok(stream_data) => stream_data,
        Err(err) => {
            state.warn(format!(
                "Failed to decode Form XObject stream while scanning PDF vector artifacts: {}",
                err
            ));
            if xobject_ref.is_some() {
                let _ = state.xobject_stack.pop();
            }
            return;
        }
    };

    let operators = match parse_content_stream(&stream_data) {
        Ok(operators) => operators,
        Err(err) => {
            state.warn(format!(
                "Failed to parse Form XObject content while scanning PDF vector artifacts: {}",
                err
            ));
            if xobject_ref.is_some() {
                let _ = state.xobject_stack.pop();
            }
            return;
        }
    };

    let parent_marked_content_stack = state.marked_content_stack.clone();
    state.graphics_state_stack.save();
    state.current_path.clear();
    {
        let graphics_state = state.graphics_state_stack.current_mut();
        graphics_state.ctm = form_matrix.multiply(&graphics_state.ctm);
    }

    collect_artifact_paths_from_operators(doc, &operators, &form_resources, state);

    state.current_path.clear();
    state.graphics_state_stack.restore();
    state.marked_content_stack = parent_marked_content_stack;

    if xobject_ref.is_some() {
        let _ = state.xobject_stack.pop();
    }
}
