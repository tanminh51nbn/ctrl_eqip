//! # Distance Estimator
//!
//! Estimates the distance to a detected person from their bounding box height.
//!
//! ## Method
//! Uses the pinhole camera model:
//! ```text
//! distance ≈ (focal_length_px × real_person_height_m) / bbox_height_px
//! ```
//!
//! ## Calibration
//! `focal_length_px` depends on your camera and can be calibrated by placing a
//! person at a known distance and measuring their bounding box height:
//! ```text
//! focal_length_px = (bbox_height_px × known_distance_m) / real_person_height_m
//! ```
//! Default values assume a typical laptop webcam (720p, ~60° FOV).
//!
//! ## Distance categories
//! | Category | Range     | Fan behaviour           |
//! |----------|-----------|-------------------------|
//! | Close    | < 1.0 m   | Gentle (avoid blow-off) |
//! | Medium   | 1–3 m     | Normal                  |
//! | Far      | 3–5 m     | Strong                  |
//! | TooFar   | > 5 m     | Off                     |

use crate::engine::BoundingBox;

/// Distance bounds (meters) defining each category.
pub const CLOSE_THRESHOLD_M: f32 = 1.0;
pub const MEDIUM_THRESHOLD_M: f32 = 3.0;
pub const FAR_THRESHOLD_M: f32 = 5.0;

/// Default average standing person height (meters).
pub const DEFAULT_PERSON_HEIGHT_M: f32 = 1.70;

/// Default focal length for a typical 720p webcam (~60° vertical FOV).
///
/// Calibrate this with `DistanceEstimator::calibrate()` for better accuracy.
pub const DEFAULT_FOCAL_LENGTH_PX: f32 = 700.0;

/// Classification of estimated distance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistanceCategory {
    /// Person is very close (< 1 m). Use gentle fan speed to avoid discomfort.
    Close,
    /// Person is at a comfortable distance (1–3 m). Normal fan speed.
    Medium,
    /// Person is far (3–5 m). Use high fan speed.
    Far,
    /// Person is too far to effectively cool (> 5 m). Turn fan off.
    TooFar,
}

impl DistanceCategory {
    /// Create a category from a distance in meters.
    pub fn from_meters(dist_m: f32) -> Self {
        if dist_m < CLOSE_THRESHOLD_M {
            Self::Close
        } else if dist_m < MEDIUM_THRESHOLD_M {
            Self::Medium
        } else if dist_m < FAR_THRESHOLD_M {
            Self::Far
        } else {
            Self::TooFar
        }
    }

    /// Returns a human-readable label for display/logging.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Close => "Close (<1m)",
            Self::Medium => "Medium (1-3m)",
            Self::Far => "Far (3-5m)",
            Self::TooFar => "Too Far (>5m)",
        }
    }
}

/// Estimates distance from bounding box height using a pinhole camera model.
pub struct DistanceEstimator {
    /// Camera focal length in pixels (calibrate per camera).
    focal_length_px: f32,
    /// Assumed real-world height of a standing person (meters).
    person_height_m: f32,
    /// Frame height (pixels) — reserved for future normalized-coordinate support.
    #[allow(dead_code)]
    frame_height_px: u32,
}

/// Result of a distance estimation.
#[derive(Debug, Clone)]
pub struct DistanceResult {
    /// Estimated distance in meters.
    pub distance_m: f32,
    /// Categorical classification of the distance.
    pub category: DistanceCategory,
}

impl DistanceEstimator {
    /// Create with default camera parameters.
    ///
    /// `frame_height_px` — height of the camera frame in pixels (e.g. 480, 720).
    pub fn new(frame_height_px: u32) -> Self {
        Self {
            focal_length_px: DEFAULT_FOCAL_LENGTH_PX,
            person_height_m: DEFAULT_PERSON_HEIGHT_M,
            frame_height_px,
        }
    }

    /// Create with custom focal length and person height.
    pub fn with_params(frame_height_px: u32, focal_length_px: f32, person_height_m: f32) -> Self {
        Self { focal_length_px, person_height_m, frame_height_px }
    }

    /// Calibrate the focal length using a reference image where a person of
    /// `real_height_m` meters is at `known_distance_m` meters from the camera
    /// and appears as `bbox_height_px` pixels tall in the frame.
    pub fn calibrate(
        &mut self,
        known_distance_m: f32,
        bbox_height_px: f32,
        real_height_m: f32,
    ) {
        self.focal_length_px = (bbox_height_px * known_distance_m) / real_height_m;
        log::info!(
            "[DistanceEstimator] Calibrated focal length: {:.1} px",
            self.focal_length_px
        );
    }

    /// Estimate the distance to a detected person from their bounding box.
    ///
    /// Uses the pixel height of the bounding box (in raw pixels, not fractions).
    pub fn estimate(&self, bbox: &BoundingBox) -> DistanceResult {
        let bbox_h = bbox.height.max(1.0); // guard against zero height
        let distance_m = (self.focal_length_px * self.person_height_m) / bbox_h;
        let distance_m = distance_m.max(0.0);

        log::debug!(
            "[DistanceEstimator] bbox_h={:.1}px → {:.2}m",
            bbox_h,
            distance_m
        );

        DistanceResult {
            distance_m,
            category: DistanceCategory::from_meters(distance_m),
        }
    }

    /// Returns the current focal length (pixels).
    pub fn focal_length_px(&self) -> f32 {
        self.focal_length_px
    }
}

