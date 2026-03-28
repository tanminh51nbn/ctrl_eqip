//! # Logic Module
//!
//! Business logic for analyzing tracking results:
//! - [`presence`]: Detects human presence and enforces the 30-second timeout.
//! - [`distance`]: Estimates distance to a person from a bounding box.
//! - [`fusion`]: Aggregates detections into a structured TrackingResult for the edge node.

pub mod distance;
pub mod fusion;
pub mod presence;

pub use distance::{DistanceCategory, DistanceEstimator};
pub use fusion::{PersonDetail, SceneAnalyzer, TrackingResult};
pub use presence::{PresenceState, PresenceTracker};
