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

/// Heuristic empirical distance estimator based on Bounding Box Geometry.
/// Uses the bounding box diagonal and an empirically derived inverse law.
pub struct DistanceEstimator {
    /// The numerator constant derived from linear regression.
    calib_numerator: f32,
    /// The intercept offset derived from linear regression.
    calib_offset: f32,
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
    /// Create with empirically tested golden constants for standard 720p/480p Webcams.
    ///
    /// `frame_height_px` — height of the camera frame in pixels (e.g. 480, 720).
    pub fn new(frame_height_px: u32) -> Self {
        Self {
            calib_numerator: 1150.0, // Calculated from 50+ data points
            calib_offset: 1.1,       // Calculated from 50+ data points
            frame_height_px,
        }
    }

    /// Create with custom empirical parameters.
    pub fn with_params(frame_height_px: u32, calib_numerator: f32, calib_offset: f32) -> Self {
        Self { calib_numerator, calib_offset, frame_height_px }
    }

    /// Calibrate the empirical numerator using a single reference measurement 
    /// of a person standing at a known distance.
    pub fn calibrate(
        &mut self,
        known_distance_m: f32,
        bbox_w_px: f32,
        bbox_h_px: f32,
    ) {
        let diag = (bbox_w_px * bbox_w_px + bbox_h_px * bbox_h_px).sqrt().max(1.0);
        // Formula: D = (Num / Diag) - Offset => Num = (D + Offset) * Diag
        self.calib_numerator = (known_distance_m + self.calib_offset) * diag;
        
        log::info!(
            "[DistanceEstimator] Calibrated numerator: {:.1}",
            self.calib_numerator
        );
    }

    /// Estimate the distance to a detected person using the Diagonal Regression Model.
    ///
    /// This "Zero-Model" mathematically compensates for sitting/crouching postures
    /// and truncated bounding boxes at extreme close range by fusing both W and H.
    pub fn estimate(&self, bbox: &BoundingBox) -> DistanceResult {
        let w = bbox.width.max(1.0); 
        let h = bbox.height.max(1.0);
        
        let diag = (w * w + h * h).sqrt();
        
        let mut distance_m = (self.calib_numerator / diag) - self.calib_offset;
        
        // Guard against negative/ridiculous bounds
        if distance_m < 0.2 {
            distance_m = 0.2;
        } else if distance_m > 20.0 {
            distance_m = 20.0;
        }

        log::debug!(
            "[DistanceEstimator] Diag={:.1}px → {:.2}m",
            diag,
            distance_m
        );

        DistanceResult {
            distance_m,
            category: DistanceCategory::from_meters(distance_m),
        }
    }
}


