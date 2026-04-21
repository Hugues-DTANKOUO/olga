use olga::model::*;

#[test]
fn bbox_right_and_bottom() {
    let bb = BoundingBox::new(0.1, 0.2, 0.3, 0.4);
    assert!((bb.right() - 0.4).abs() < f32::EPSILON);
    assert!((bb.bottom() - 0.6).abs() < f32::EPSILON);
}

#[test]
fn bbox_center() {
    let bb = BoundingBox::new(0.0, 0.0, 1.0, 1.0);
    let c = bb.center();
    assert!((c.x - 0.5).abs() < f32::EPSILON);
    assert!((c.y - 0.5).abs() < f32::EPSILON);
}

#[test]
fn bbox_area() {
    let bb = BoundingBox::new(0.0, 0.0, 0.5, 0.5);
    assert!((bb.area() - 0.25).abs() < f32::EPSILON);
}

#[test]
fn bbox_overlaps_true() {
    let a = BoundingBox::new(0.0, 0.0, 0.5, 0.5);
    let b = BoundingBox::new(0.25, 0.25, 0.5, 0.5);
    assert!(a.overlaps(&b));
    assert!(b.overlaps(&a));
}

#[test]
fn bbox_overlaps_false_horizontal() {
    let a = BoundingBox::new(0.0, 0.0, 0.3, 0.3);
    let b = BoundingBox::new(0.5, 0.0, 0.3, 0.3);
    assert!(!a.overlaps(&b));
}

#[test]
fn bbox_overlaps_false_vertical() {
    let a = BoundingBox::new(0.0, 0.0, 0.3, 0.3);
    let b = BoundingBox::new(0.0, 0.5, 0.3, 0.3);
    assert!(!a.overlaps(&b));
}

#[test]
fn bbox_overlaps_touching_edges_is_false() {
    let a = BoundingBox::new(0.0, 0.0, 0.5, 0.5);
    let b = BoundingBox::new(0.5, 0.0, 0.5, 0.5);
    assert!(!a.overlaps(&b));
}

#[test]
fn bbox_intersection_some() {
    let a = BoundingBox::new(0.0, 0.0, 0.6, 0.6);
    let b = BoundingBox::new(0.4, 0.4, 0.6, 0.6);
    let inter = a.intersection(&b).unwrap();
    assert!((inter.x - 0.4).abs() < 1e-6);
    assert!((inter.y - 0.4).abs() < 1e-6);
    assert!((inter.width - 0.2).abs() < 1e-6);
    assert!((inter.height - 0.2).abs() < 1e-6);
}

#[test]
fn bbox_intersection_none() {
    let a = BoundingBox::new(0.0, 0.0, 0.3, 0.3);
    let b = BoundingBox::new(0.5, 0.5, 0.3, 0.3);
    assert!(a.intersection(&b).is_none());
}

#[test]
fn bbox_contains_true() {
    let outer = BoundingBox::new(0.0, 0.0, 1.0, 1.0);
    let inner = BoundingBox::new(0.1, 0.1, 0.5, 0.5);
    assert!(outer.contains(&inner));
}

#[test]
fn bbox_contains_false() {
    let a = BoundingBox::new(0.0, 0.0, 0.5, 0.5);
    let b = BoundingBox::new(0.3, 0.3, 0.5, 0.5);
    assert!(!a.contains(&b));
}

#[test]
fn bbox_contains_self() {
    let a = BoundingBox::new(0.1, 0.2, 0.3, 0.4);
    assert!(a.contains(&a));
}

#[test]
fn bbox_contains_point() {
    let bb = BoundingBox::new(0.1, 0.1, 0.5, 0.5);
    assert!(bb.contains_point(&Point::new(0.3, 0.3)));
    assert!(!bb.contains_point(&Point::new(0.0, 0.0)));
    assert!(bb.contains_point(&Point::new(0.1, 0.1)));
}

#[test]
fn bbox_merge() {
    let a = BoundingBox::new(0.1, 0.2, 0.3, 0.3);
    let b = BoundingBox::new(0.5, 0.1, 0.2, 0.4);
    let m = a.merge(&b);
    assert!((m.x - 0.1).abs() < 1e-6);
    assert!((m.y - 0.1).abs() < 1e-6);
    assert!((m.right() - 0.7).abs() < 1e-6);
    assert!((m.bottom() - 0.5).abs() < 1e-6);
}

#[test]
fn bbox_merge_is_commutative() {
    let a = BoundingBox::new(0.0, 0.0, 0.3, 0.3);
    let b = BoundingBox::new(0.5, 0.5, 0.2, 0.2);
    let m1 = a.merge(&b);
    let m2 = b.merge(&a);
    assert!((m1.x - m2.x).abs() < 1e-6);
    assert!((m1.y - m2.y).abs() < 1e-6);
    assert!((m1.width - m2.width).abs() < 1e-6);
    assert!((m1.height - m2.height).abs() < 1e-6);
}

#[test]
fn bbox_distance_overlapping_is_zero() {
    let a = BoundingBox::new(0.0, 0.0, 0.5, 0.5);
    let b = BoundingBox::new(0.3, 0.3, 0.5, 0.5);
    assert!((a.distance(&b)).abs() < 1e-6);
}

#[test]
fn bbox_distance_horizontal_gap() {
    let a = BoundingBox::new(0.0, 0.0, 0.3, 0.3);
    let b = BoundingBox::new(0.5, 0.0, 0.3, 0.3);
    assert!((a.distance(&b) - 0.2).abs() < 1e-6);
}

#[test]
fn bbox_distance_diagonal() {
    let a = BoundingBox::new(0.0, 0.0, 0.1, 0.1);
    let b = BoundingBox::new(0.4, 0.3, 0.1, 0.1);
    let expected = (0.3_f32.powi(2) + 0.2_f32.powi(2)).sqrt();
    assert!((a.distance(&b) - expected).abs() < 1e-5);
}

#[test]
fn bbox_horizontally_aligned() {
    let a = BoundingBox::new(0.1, 0.0, 0.5, 0.1);
    let b = BoundingBox::new(0.2, 0.2, 0.5, 0.1);
    assert!(a.horizontally_aligned(&b, 0.5));
    assert!(!a.horizontally_aligned(&b, 0.9));
}

#[test]
fn bbox_left_aligned() {
    let a = BoundingBox::new(0.10, 0.0, 0.5, 0.1);
    let b = BoundingBox::new(0.11, 0.2, 0.5, 0.1);
    assert!(a.left_aligned(&b, 0.02));
    assert!(!a.left_aligned(&b, 0.005));
}

#[test]
fn bbox_valid() {
    assert!(BoundingBox::new(0.0, 0.0, 1.0, 1.0).is_valid());
    assert!(BoundingBox::new(0.5, 0.5, 0.5, 0.5).is_valid());
}

#[test]
fn bbox_invalid_negative() {
    assert!(!BoundingBox::new(-0.1, 0.0, 0.5, 0.5).is_valid());
}

#[test]
fn bbox_invalid_overflow() {
    assert!(!BoundingBox::new(0.6, 0.0, 0.5, 0.5).is_valid());
}

#[test]
fn point_distance() {
    let a = Point::new(0.0, 0.0);
    let b = Point::new(0.3, 0.4);
    assert!((a.distance(&b) - 0.5).abs() < 1e-6);
}

#[test]
fn page_dimensions_no_rotation() {
    let media = RawBox::new(0.0, 0.0, 612.0, 792.0);
    let pd = PageDimensions::new(media, None, 0);
    assert!((pd.effective_width - 612.0).abs() < 1e-6);
    assert!((pd.effective_height - 792.0).abs() < 1e-6);
}

#[test]
fn page_dimensions_rotation_90() {
    let media = RawBox::new(0.0, 0.0, 612.0, 792.0);
    let pd = PageDimensions::new(media, None, 90);
    assert!((pd.effective_width - 792.0).abs() < 1e-6);
    assert!((pd.effective_height - 612.0).abs() < 1e-6);
}

#[test]
fn page_dimensions_rotation_180() {
    let media = RawBox::new(0.0, 0.0, 612.0, 792.0);
    let pd = PageDimensions::new(media, None, 180);
    assert!((pd.effective_width - 612.0).abs() < 1e-6);
    assert!((pd.effective_height - 792.0).abs() < 1e-6);
}

#[test]
fn page_dimensions_rotation_270() {
    let media = RawBox::new(0.0, 0.0, 612.0, 792.0);
    let pd = PageDimensions::new(media, None, 270);
    assert!((pd.effective_width - 792.0).abs() < 1e-6);
    assert!((pd.effective_height - 612.0).abs() < 1e-6);
}

#[test]
fn page_dimensions_invalid_rotation_defaults_to_zero() {
    let media = RawBox::new(0.0, 0.0, 612.0, 792.0);
    let pd = PageDimensions::new(media, None, 45);
    assert_eq!(pd.rotation, 0);
    assert!((pd.effective_width - 612.0).abs() < 1e-6);
}

#[test]
fn page_dimensions_crop_box_overrides_media_box() {
    let media = RawBox::new(0.0, 0.0, 612.0, 792.0);
    let crop = RawBox::new(50.0, 50.0, 512.0, 692.0);
    let pd = PageDimensions::new(media, Some(crop), 0);
    assert!((pd.effective_width - 512.0).abs() < 1e-6);
    assert!((pd.effective_height - 692.0).abs() < 1e-6);
}

#[test]
fn normalize_point_no_rotation_no_flip() {
    let media = RawBox::new(0.0, 0.0, 100.0, 200.0);
    let pd = PageDimensions::new(media, None, 0);
    let p = pd.normalize_point(50.0, 100.0, false);
    assert!((p.x - 0.5).abs() < 1e-6);
    assert!((p.y - 0.5).abs() < 1e-6);
}

#[test]
fn normalize_point_with_y_flip() {
    let media = RawBox::new(0.0, 0.0, 100.0, 200.0);
    let pd = PageDimensions::new(media, None, 0);
    let p = pd.normalize_point(50.0, 0.0, true);
    assert!((p.x - 0.5).abs() < 1e-6);
    assert!((p.y - 1.0).abs() < 1e-6);
}

#[test]
fn normalize_point_with_y_flip_top() {
    let media = RawBox::new(0.0, 0.0, 100.0, 200.0);
    let pd = PageDimensions::new(media, None, 0);
    let p = pd.normalize_point(50.0, 200.0, true);
    assert!((p.x - 0.5).abs() < 1e-6);
    assert!((p.y - 0.0).abs() < 1e-6);
}
