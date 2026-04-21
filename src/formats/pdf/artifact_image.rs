use std::collections::{BTreeMap, HashSet};

use pdf_oxide::PdfDocument;
use pdf_oxide::content::{Operator, parse_content_stream};
use pdf_oxide::extractors::{ImageData, PdfImage};
use pdf_oxide::object::Object;
use pdf_oxide::object::ObjectRef;

use crate::error::{Warning, WarningKind};

use super::marked_content::{
    MarkedContentContext, active_marked_content_artifact, active_marked_content_mcid,
    build_marked_content_context, describe_artifact_kind, marked_content_context_for_tag,
};
use super::resources::{load_page_resources, resolve_named_xobject, resolve_object};
use super::tagged::{
    ImageAltBinding, ImageAltTextResolution, TaggedImageAppearance, TaggedPageContext,
    bind_alt_text_to_image_appearances,
};
use super::types::RawImage;

pub(super) fn convert_images(
    images: &[PdfImage],
    page_ctx: Option<&TaggedPageContext>,
    appearances: Option<&[TaggedImageAppearance]>,
    page_num: u32,
    warnings: &mut Vec<Warning>,
) -> Vec<RawImage> {
    let alt_bindings = bind_alt_text_to_image_appearances(page_ctx, appearances, images.len());
    if let ImageAltBinding::CountMismatch {
        appearance_count,
        image_count,
    } = &alt_bindings
    {
        warnings.push(Warning {
            kind: WarningKind::PartialExtraction,
            message: format!(
                "Tagged PDF image appearance scan found {} image invocations but the image extractor returned {} images; leaving image alt_text unset for this page",
                appearance_count, image_count
            ),
            page: Some(page_num),
        });
    }

    let mut warned_ambiguous_mcids = HashSet::new();
    let mut filtered_artifact_counts = BTreeMap::new();
    let mut filtered_artifact_total = 0usize;
    let raw_images: Vec<RawImage> = images
        .iter()
        .enumerate()
        .filter_map(|(idx, img)| {
            if let Some(appearance) = appearances.and_then(|items| items.get(idx))
                && appearance.is_artifact
            {
                filtered_artifact_total += 1;
                *filtered_artifact_counts
                    .entry(describe_artifact_kind(appearance.artifact_type.as_ref()).to_string())
                    .or_insert(0usize) += 1;
                return None;
            }

            let bbox = img.bbox()?;

            let (data, format) = match img.data() {
                ImageData::Jpeg(bytes) => (bytes.clone(), crate::model::ImageFormat::Jpeg),
                ImageData::Raw { pixels, .. } => {
                    (pixels.clone(), crate::model::ImageFormat::Unknown)
                }
            };

            let alt_text = match &alt_bindings {
                ImageAltBinding::Resolved(resolutions) => match resolutions.get(idx) {
                    Some(ImageAltTextResolution::Unique(value)) => Some(value.clone()),
                    Some(ImageAltTextResolution::Ambiguous(_)) => {
                        if let Some(mcid) = appearances
                            .and_then(|items| items.get(idx))
                            .and_then(|appearance| appearance.mcid)
                        {
                            if warned_ambiguous_mcids.insert(mcid) {
                                warnings.push(Warning {
                                    kind: WarningKind::PartialExtraction,
                                    message: format!(
                                        "Tagged PDF associates multiple alt-text candidates with image MCID {} on this page; leaving image alt_text unset",
                                        mcid
                                    ),
                                    page: Some(page_num),
                                });
                            }
                        } else if warned_ambiguous_mcids.insert(u32::MAX) {
                            warnings.push(Warning {
                                kind: WarningKind::PartialExtraction,
                                message:
                                    "Tagged PDF image alt-text binding was ambiguous on this page; leaving image alt_text unset"
                                        .to_string(),
                                page: Some(page_num),
                            });
                        }
                        None
                    }
                    _ => None,
                },
                _ => None,
            };

            Some(RawImage {
                data,
                format,
                alt_text,
                mcid: appearances
                    .and_then(|items| items.get(idx))
                    .and_then(|appearance| appearance.mcid),
                raw_x: bbox.x,
                raw_y: bbox.y,
                raw_width: bbox.width,
                raw_height: bbox.height,
            })
        })
        .collect();

    if !filtered_artifact_counts.is_empty() {
        let details = filtered_artifact_counts
            .into_iter()
            .map(|(label, count)| format!("{} {}", count, label))
            .collect::<Vec<_>>()
            .join(", ");
        warnings.push(Warning {
            kind: WarningKind::FilteredArtifact,
            message: format!(
                "Filtered {} PDF image artifact(s) on this page ({})",
                filtered_artifact_total, details
            ),
            page: Some(page_num),
        });
    }

    raw_images
}

pub(super) fn scan_page_image_appearances(
    doc: &mut PdfDocument,
    page_idx: usize,
    warnings: &mut Vec<Warning>,
) -> Option<Vec<TaggedImageAppearance>> {
    let page_num = page_idx as u32;
    let resources = match load_page_resources(doc, page_idx) {
        Ok(resources) => resources,
        Err(err) => {
            warnings.push(Warning {
                kind: WarningKind::PartialExtraction,
                message: format!(
                    "Failed to resolve page resources while scanning tagged image appearances: {}",
                    err
                ),
                page: Some(page_num),
            });
            return None;
        }
    };

    let content_data = match doc.get_page_content_data(page_idx) {
        Ok(content_data) => content_data,
        Err(err) => {
            warnings.push(Warning {
                kind: WarningKind::PartialExtraction,
                message: format!(
                    "Failed to read page content while scanning tagged image appearances: {}",
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
                    "Failed to parse page content while scanning tagged image appearances: {}",
                    err
                ),
                page: Some(page_num),
            });
            return None;
        }
    };

    let mut state = ImageAppearanceScanState::new(page_num, warnings);
    collect_image_appearances_from_operators(doc, &operators, &resources, &mut state);
    Some(state.appearances)
}

struct ImageAppearanceScanState<'a> {
    xobject_stack: Vec<ObjectRef>,
    marked_content_stack: Vec<MarkedContentContext>,
    appearances: Vec<TaggedImageAppearance>,
    warnings: &'a mut Vec<Warning>,
    page_num: u32,
}

impl<'a> ImageAppearanceScanState<'a> {
    fn new(page_num: u32, warnings: &'a mut Vec<Warning>) -> Self {
        Self {
            xobject_stack: Vec::new(),
            marked_content_stack: Vec::new(),
            appearances: Vec::new(),
            warnings,
            page_num,
        }
    }

    fn warn(&mut self, message: String) {
        self.warnings.push(Warning {
            kind: WarningKind::PartialExtraction,
            message,
            page: Some(self.page_num),
        });
    }
}

fn collect_image_appearances_from_operators(
    doc: &mut PdfDocument,
    operators: &[Operator],
    resources: &Object,
    state: &mut ImageAppearanceScanState<'_>,
) {
    for operator in operators {
        match operator {
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
            Operator::InlineImage { .. } => {
                let active_artifact = active_marked_content_artifact(&state.marked_content_stack);
                state.appearances.push(TaggedImageAppearance {
                    mcid: active_marked_content_mcid(&state.marked_content_stack),
                    is_artifact: active_artifact.is_some(),
                    artifact_type: active_artifact.and_then(|ctx| ctx.artifact_type.clone()),
                });
            }
            Operator::Do { name } => {
                let Some((xobject, xobject_ref)) =
                    resolve_named_xobject(doc, resources, name.as_str())
                else {
                    continue;
                };

                match xobject
                    .as_dict()
                    .and_then(|dict| dict.get("Subtype"))
                    .and_then(Object::as_name)
                {
                    Some("Image") => {
                        let active_artifact =
                            active_marked_content_artifact(&state.marked_content_stack);
                        state.appearances.push(TaggedImageAppearance {
                            mcid: active_marked_content_mcid(&state.marked_content_stack),
                            is_artifact: active_artifact.is_some(),
                            artifact_type: active_artifact
                                .and_then(|ctx| ctx.artifact_type.clone()),
                        });
                    }
                    Some("Form") => {
                        scan_form_xobject_image_appearances(
                            doc,
                            &xobject,
                            xobject_ref,
                            resources,
                            state,
                        );
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

fn scan_form_xobject_image_appearances(
    doc: &mut PdfDocument,
    xobject: &Object,
    xobject_ref: Option<ObjectRef>,
    parent_resources: &Object,
    state: &mut ImageAppearanceScanState<'_>,
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

    let stream_data = match xobject.decode_stream_data() {
        Ok(stream_data) => stream_data,
        Err(err) => {
            state.warn(format!(
                "Failed to decode Form XObject stream while scanning tagged image appearances: {}",
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
                "Failed to parse Form XObject content while scanning tagged image appearances: {}",
                err
            ));
            if xobject_ref.is_some() {
                let _ = state.xobject_stack.pop();
            }
            return;
        }
    };

    let parent_marked_content_stack = state.marked_content_stack.clone();
    collect_image_appearances_from_operators(doc, &operators, &form_resources, state);
    state.marked_content_stack = parent_marked_content_stack;

    if xobject_ref.is_some() {
        let _ = state.xobject_stack.pop();
    }
}
