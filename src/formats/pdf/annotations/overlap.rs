use crate::model::geometry::BoundingBox;

pub(super) fn bbox_overlap_sufficient(prim_bbox: &BoundingBox, annot_bbox: &BoundingBox) -> bool {
    let prim_left = prim_bbox.x;
    let prim_right = prim_bbox.x + prim_bbox.width;
    let annot_left = annot_bbox.x;
    let annot_right = annot_bbox.x + annot_bbox.width;

    let overlap_left = prim_left.max(annot_left);
    let overlap_right = prim_right.min(annot_right);
    let h_overlap = (overlap_right - overlap_left).max(0.0);

    if prim_bbox.width <= 0.0 || h_overlap / prim_bbox.width < 0.5 {
        return false;
    }

    let prim_top = prim_bbox.y;
    let prim_bottom = prim_bbox.y + prim_bbox.height;
    let annot_top = annot_bbox.y;
    let annot_bottom = annot_bbox.y + annot_bbox.height;

    let overlap_top = prim_top.max(annot_top);
    let overlap_bottom = prim_bottom.min(annot_bottom);

    overlap_bottom > overlap_top
}

pub(super) fn overlap_score(prim_bbox: &BoundingBox, annot_bbox: &BoundingBox) -> Option<f32> {
    if !bbox_overlap_sufficient(prim_bbox, annot_bbox) {
        return None;
    }

    let prim_left = prim_bbox.x;
    let prim_right = prim_bbox.x + prim_bbox.width;
    let annot_left = annot_bbox.x;
    let annot_right = annot_bbox.x + annot_bbox.width;
    let overlap_left = prim_left.max(annot_left);
    let overlap_right = prim_right.min(annot_right);
    let h_overlap = (overlap_right - overlap_left).max(0.0);
    let h_ratio = if prim_bbox.width > 0.0 {
        h_overlap / prim_bbox.width
    } else {
        0.0
    };

    let prim_cx = prim_bbox.x + prim_bbox.width / 2.0;
    let prim_cy = prim_bbox.y + prim_bbox.height / 2.0;
    let annot_cx = annot_bbox.x + annot_bbox.width / 2.0;
    let annot_cy = annot_bbox.y + annot_bbox.height / 2.0;
    let dx = (prim_cx - annot_cx).abs();
    let dy = (prim_cy - annot_cy).abs();
    let proximity = 1.0 / (1.0 + dx + dy);

    Some(h_ratio + proximity * 0.25)
}
