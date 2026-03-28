use ctrl_eqip::logic::distance::{DistanceEstimator, DistanceCategory};
use ctrl_eqip::engine::detector::BoundingBox;

fn make_bbox(height_px: f32) -> BoundingBox {
    BoundingBox { x: 0.0, y: 0.0, width: 50.0, height: height_px, confidence: 0.9, class_id: 0 }
}

#[test]
fn close_distance() {
    let _est = DistanceEstimator::new(480);
    // focal=700, person=1.70 -> distance = 700*1.70/bbox_h
    let est2 = DistanceEstimator::with_params(480, 200.0, 1.70);
    let r = est2.estimate(&make_bbox(400.0)); // 200*1.7/400 = 0.85m
    assert_eq!(r.category, DistanceCategory::Close);
}

#[test]
fn medium_distance() {
    let est = DistanceEstimator::with_params(480, 700.0, 1.70);
    // 700*1.7 / bbox_h = 2m -> bbox_h = 595
    let r = est.estimate(&make_bbox(595.0));
    assert_eq!(r.category, DistanceCategory::Medium);
}

#[test]
fn far_distance() {
    let est = DistanceEstimator::with_params(480, 700.0, 1.70);
    // target 4m -> bbox_h = 700*1.7/4 = 297.5
    let r = est.estimate(&make_bbox(298.0));
    assert_eq!(r.category, DistanceCategory::Far);
}

#[test]
fn too_far_distance() {
    let est = DistanceEstimator::with_params(480, 700.0, 1.70);
    // target 6m -> bbox_h = 700*1.7/6 = 198
    let r = est.estimate(&make_bbox(100.0));
    assert_eq!(r.category, DistanceCategory::TooFar);
}

#[test]
fn category_from_meters() {
    assert_eq!(DistanceCategory::from_meters(0.5), DistanceCategory::Close);
    assert_eq!(DistanceCategory::from_meters(2.0), DistanceCategory::Medium);
    assert_eq!(DistanceCategory::from_meters(4.0), DistanceCategory::Far);
    assert_eq!(DistanceCategory::from_meters(10.0), DistanceCategory::TooFar);
}
