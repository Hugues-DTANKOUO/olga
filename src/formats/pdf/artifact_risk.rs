use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use crate::error::{Warning, WarningKind};
use crate::model::{BoundingBox, ImageFormat, PageDimensions};

use super::artifact_text::{ArtifactBand, classify_artifact_band};
use super::tagged::TaggedPageContext;
use super::types::{RawImage, RawPath};

const MIN_REPEATED_ARTIFACT_OCCURRENCES: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct RepeatedVectorArtifactSignature {
    pub(super) kind: &'static str,
    pub(super) band: ArtifactBand,
    pub(super) qx: i16,
    pub(super) qy: i16,
    pub(super) qw: i16,
    pub(super) qh: i16,
    pub(super) style: u32,
}

#[derive(Debug, Clone)]
pub(super) struct RepeatedVectorArtifactCandidate {
    pub(super) page: u32,
    pub(super) band: ArtifactBand,
    pub(super) signature: RepeatedVectorArtifactSignature,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct RepeatedImageArtifactSignature {
    pub(super) band: ArtifactBand,
    pub(super) qx: i16,
    pub(super) qy: i16,
    pub(super) qw: i16,
    pub(super) qh: i16,
    pub(super) format: ImageFormat,
    pub(super) data_fingerprint: u64,
}

#[derive(Debug, Clone)]
pub(super) struct RepeatedImageArtifactCandidate {
    pub(super) page: u32,
    pub(super) band: ArtifactBand,
    pub(super) signature: RepeatedImageArtifactSignature,
}

#[derive(Debug, Default)]
pub(super) struct ArtifactRiskCollector {
    repeated_vector_candidates: Vec<RepeatedVectorArtifactCandidate>,
    repeated_image_candidates: Vec<RepeatedImageArtifactCandidate>,
}

impl ArtifactRiskCollector {
    pub(super) fn note_paths(
        &mut self,
        paths: &[RawPath],
        page_dims: &PageDimensions,
        page: u32,
        use_structure_reading_order: bool,
        page_ctx: Option<&TaggedPageContext>,
    ) {
        if !(use_structure_reading_order
            && page_ctx.is_some_and(TaggedPageContext::has_reading_items))
        {
            return;
        }

        for path in paths {
            let Some(signature) = repeated_vector_artifact_signature(path, page_dims) else {
                continue;
            };
            self.repeated_vector_candidates
                .push(RepeatedVectorArtifactCandidate {
                    page,
                    band: signature.band,
                    signature,
                });
        }
    }

    #[allow(dead_code)] // Image extraction disabled; kept for future re-enablement.
    pub(super) fn note_images(
        &mut self,
        images: &[RawImage],
        page_dims: &PageDimensions,
        page: u32,
        use_structure_reading_order: bool,
        page_ctx: Option<&TaggedPageContext>,
    ) {
        if !(use_structure_reading_order
            && page_ctx.is_some_and(TaggedPageContext::has_reading_items))
        {
            return;
        }

        for image in images {
            if image
                .mcid
                .is_some_and(|mcid| page_ctx.is_some_and(|ctx| ctx.references_mcid(mcid)))
            {
                continue;
            }

            let Some(signature) = repeated_image_artifact_signature(image, page_dims) else {
                continue;
            };
            self.repeated_image_candidates
                .push(RepeatedImageArtifactCandidate {
                    page,
                    band: signature.band,
                    signature,
                });
        }
    }

    pub(super) fn emit_warnings(&self, warnings: &mut Vec<Warning>) {
        emit_repeated_vector_artifact_risk_warnings(&self.repeated_vector_candidates, warnings);
        emit_repeated_image_artifact_risk_warnings(&self.repeated_image_candidates, warnings);
    }
}

pub(super) fn repeated_vector_artifact_signature(
    path: &RawPath,
    page_dims: &PageDimensions,
) -> Option<RepeatedVectorArtifactSignature> {
    let (kind, bbox, style, suspicious) = repeated_vector_artifact_parts(path, page_dims)?;
    suspicious.then_some(RepeatedVectorArtifactSignature {
        kind,
        band: classify_artifact_band(bbox)?,
        qx: quantize_norm(bbox.x),
        qy: quantize_norm(bbox.y),
        qw: quantize_norm(bbox.width),
        qh: quantize_norm(bbox.height),
        style,
    })
}

fn repeated_vector_artifact_parts(
    path: &RawPath,
    page_dims: &PageDimensions,
) -> Option<(&'static str, BoundingBox, u32, bool)> {
    match path {
        RawPath::Line {
            x0,
            y0,
            x1,
            y1,
            thickness,
            color_r,
            color_g,
            color_b,
        } => {
            let min_x = x0.min(*x1);
            let min_y = y0.min(*y1);
            let width = (x1 - x0).abs();
            let height = (y1 - y0).abs();
            let bbox = page_dims.normalize_bbox(min_x, min_y, width, height, true);
            let length = bbox.width.max(bbox.height);
            let thin = bbox
                .width
                .min(bbox.height)
                .max(thickness / page_dims.effective_width.max(1.0));
            let suspicious =
                length >= 0.25 && thin <= 0.02 && classify_artifact_band(bbox).is_some();
            Some((
                "line",
                bbox,
                pack_rgb(*color_r, *color_g, *color_b),
                suspicious,
            ))
        }
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
        } => {
            let bbox = page_dims.normalize_bbox(*x, *y, *width, *height, true);
            let fill = if let (Some(r), Some(g), Some(b)) = (fill_r, fill_g, fill_b) {
                pack_rgb(*r, *g, *b)
            } else {
                0
            };
            let stroke = if let (Some(r), Some(g), Some(b)) = (stroke_r, stroke_g, stroke_b) {
                pack_rgb(*r, *g, *b)
            } else {
                0
            };
            let suspicious = classify_artifact_band(bbox).is_some()
                && ((bbox.width >= 0.20 && bbox.height <= 0.05)
                    || (bbox.height >= 0.20 && bbox.width <= 0.05)
                    || (bbox.width >= 0.40 && bbox.height >= 0.04))
                && (*stroke_thickness <= 8.0 || fill != 0);
            Some(("rect", bbox, fill ^ stroke.rotate_left(1), suspicious))
        }
    }
}

pub(super) fn emit_repeated_vector_artifact_risk_warnings(
    candidates: &[RepeatedVectorArtifactCandidate],
    warnings: &mut Vec<Warning>,
) {
    let mut occurrences: HashMap<RepeatedVectorArtifactSignature, HashSet<u32>> = HashMap::new();
    for candidate in candidates {
        occurrences
            .entry(candidate.signature.clone())
            .or_default()
            .insert(candidate.page);
    }

    let mut warned = HashSet::new();
    for candidate in candidates {
        let Some(pages) = occurrences.get(&candidate.signature) else {
            continue;
        };
        if pages.len() < MIN_REPEATED_ARTIFACT_OCCURRENCES {
            continue;
        }
        if !warned.insert((candidate.page, candidate.signature.clone())) {
            continue;
        }

        warnings.push(Warning {
            kind: WarningKind::SuspectedArtifact,
            message: format!(
                "Detected repeated untagged PDF {} pattern in the {} page band across {} pages; left unfiltered because the content stream does not mark it as /Artifact",
                candidate.signature.kind,
                match candidate.band {
                    ArtifactBand::Top => "top",
                    ArtifactBand::Bottom => "bottom",
                },
                pages.len()
            ),
            page: Some(candidate.page),
        });
    }
}

pub(super) fn repeated_image_artifact_signature(
    image: &RawImage,
    page_dims: &PageDimensions,
) -> Option<RepeatedImageArtifactSignature> {
    let bbox = page_dims.normalize_bbox(
        image.raw_x,
        image.raw_y,
        image.raw_width,
        image.raw_height,
        true,
    );
    let band = classify_artifact_band(bbox)?;
    let prominent = bbox.area() >= 0.0015 || bbox.width.max(bbox.height) >= 0.05;
    prominent.then(|| RepeatedImageArtifactSignature {
        band,
        qx: quantize_norm(bbox.x),
        qy: quantize_norm(bbox.y),
        qw: quantize_norm(bbox.width),
        qh: quantize_norm(bbox.height),
        format: image.format,
        data_fingerprint: image_data_fingerprint(&image.data),
    })
}

pub(super) fn emit_repeated_image_artifact_risk_warnings(
    candidates: &[RepeatedImageArtifactCandidate],
    warnings: &mut Vec<Warning>,
) {
    let mut occurrences: HashMap<RepeatedImageArtifactSignature, HashSet<u32>> = HashMap::new();
    for candidate in candidates {
        occurrences
            .entry(candidate.signature.clone())
            .or_default()
            .insert(candidate.page);
    }

    let mut warned = HashSet::new();
    for candidate in candidates {
        let Some(pages) = occurrences.get(&candidate.signature) else {
            continue;
        };
        if pages.len() < MIN_REPEATED_ARTIFACT_OCCURRENCES {
            continue;
        }
        if !warned.insert((candidate.page, candidate.signature.clone())) {
            continue;
        }

        warnings.push(Warning {
            kind: WarningKind::SuspectedArtifact,
            message: format!(
                "Detected repeated untagged PDF image pattern in the {} page band across {} pages; left unfiltered because the content stream does not mark it as /Artifact",
                match candidate.band {
                    ArtifactBand::Top => "top",
                    ArtifactBand::Bottom => "bottom",
                },
                pages.len()
            ),
            page: Some(candidate.page),
        });
    }
}

fn image_data_fingerprint(data: &[u8]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    data.len().hash(&mut hasher);
    data.iter()
        .take(256)
        .for_each(|byte| byte.hash(&mut hasher));
    hasher.finish()
}

fn quantize_norm(value: f32) -> i16 {
    (value.clamp(0.0, 1.0) * 1000.0).round() as i16
}

fn pack_rgb(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}
