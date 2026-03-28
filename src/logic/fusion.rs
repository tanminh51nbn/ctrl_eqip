//! # Scene Analyzer (Logic Module)
//!
//! Processes AI detections to analyze the presence of people, their count,
//! individual distances, and identifies the closest person.
//! Replaces old temperature-based fusion logic.

use crate::engine::detector::BoundingBox;
use crate::logic::distance::{DistanceCategory, DistanceEstimator};

/// Detailed information about a detected person within the frame.
#[derive(Debug, Clone, PartialEq)]
pub struct PersonDetail {
    /// Bounding box of the person.
    pub bbox: BoundingBox,
    /// Estimated distance to this person in meters.
    pub distance_m: f32,
    /// Categorial distance (Close, Medium, Far, etc.) for UI logging
    pub category: DistanceCategory,
}

/// Output of the logic analysis step.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackingResult {
    /// True if there is at least one person detected.
    pub has_person: bool,
    /// Total number of detected people.
    pub person_count: usize,
    /// Detailed list of all people linking their bbox to their distance.
    pub people: Vec<PersonDetail>,
    /// Distance of the closest person (if any) in meters.
    pub closest_distance_m: Option<f32>,
}

#[derive(Default, Debug, Clone)]
pub struct SceneAnalyzer {
    pub frame_height_px: u32,
}

impl SceneAnalyzer {
    /// Create a new analyzer with a defined frame height.
    pub fn new(frame_height_px: u32) -> Self {
        Self { frame_height_px }
    }

    /// Analyze a frame's detections to produce the final `TrackingResult`.
    ///
    /// # Arguments
    /// - `boxes` — List of bounding boxes extracted from the AI engine.
    /// - `estimator` — Distance estimator tuned for the current camera setup.
    /// - `person_class_id` — The class ID representing a person (e.g. 0 for COCO).
    pub fn analyze(
        &self,
        boxes: &[BoundingBox],
        estimator: &DistanceEstimator,
        person_class_id: usize,
    ) -> TrackingResult {
        let mut people = Vec::new();
        let mut closest_distance_m: Option<f32> = None;

        for bbox in boxes {
            if bbox.class_id == person_class_id {
                let dist_res = estimator.estimate(bbox);
                let distance = dist_res.distance_m;

                people.push(PersonDetail {
                    bbox: bbox.clone(),
                    distance_m: distance,
                    category: dist_res.category,
                });

                match closest_distance_m {
                    None => closest_distance_m = Some(distance),
                    Some(closest) => {
                        if distance < closest {
                            closest_distance_m = Some(distance);
                        }
                    }
                }
            }
        }

        TrackingResult {
            has_person: !people.is_empty(),
            person_count: people.len(),
            people,
            closest_distance_m,
        }
    }
}

