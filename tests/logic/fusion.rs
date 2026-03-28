use ctrl_eqip::logic::fusion::SceneAnalyzer;
use ctrl_eqip::logic::distance::DistanceEstimator;
use ctrl_eqip::engine::detector::BoundingBox;

fn make_bbox(x: f32, y: f32, w: f32, h: f32, class_id: usize) -> BoundingBox {
    BoundingBox {
        x,
        y,
        width: w,
        height: h,
        confidence: 0.9,
        class_id,
    }
}

#[test]
fn test_analyze_empty() {
    let analyzer = SceneAnalyzer::new(720);
    let estimator = DistanceEstimator::new(720);
    let result = analyzer.analyze(&[], &estimator, 0);

    assert!(!result.has_person);
    assert_eq!(result.person_count, 0);
    assert!(result.closest_distance_m.is_none());
    assert!(result.people.is_empty());
}

#[test]
fn test_analyze_people() {
    let analyzer = SceneAnalyzer::new(720);
    let estimator = DistanceEstimator::new(720); // default gives 700*1.7/bbox_h

    let boxes = vec![
        make_bbox(10.0, 10.0, 50.0, 119.0, 0), // distance = 700*1.7/119 = 10m
        make_bbox(100.0, 100.0, 200.0, 595.0, 0), // distance = 700*1.7/595 = 2.0m
        make_bbox(0.0, 0.0, 10.0, 10.0, 1),    // ignored, wrong class
    ];

    let result = analyzer.analyze(&boxes, &estimator, 0);

    assert!(result.has_person);
    assert_eq!(result.person_count, 2);
    assert_eq!(result.people.len(), 2);
    
    // Check closest distance
    let expected_closest = 2.0;
    assert!((result.closest_distance_m.unwrap() - expected_closest).abs() < 0.1);
    
    // Check detailed linkage
    assert_eq!(result.people[0].bbox.height, 119.0);
    assert!((result.people[0].distance_m - 10.0).abs() < 0.1);

    assert_eq!(result.people[1].bbox.height, 595.0);
    assert!((result.people[1].distance_m - 2.0).abs() < 0.1);
}
