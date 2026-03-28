//! # ctrl_eqip
//!
//! A high-performance Edge AI framework for human detection and device control.
//!
//! ## Architecture
//! - **AI Node** (Runs on PC/Laptop/Edge PC): Executes body detection models,
//!   estimates interpersonal distances, processes business logic, and sends
//!   commands to microcontrollers via serial communication.
//! - **Control Node** (ESP32/STM32): Receives binary commands from the AI Node
//!   to toggle equipment (Fans, Relays, LEDs) and reports sensor status.
//!
//! ## Core Pipeline
//! The library provides a `DetectorPipeline` implementing a multi-threaded
//! Producer-Consumer model optimized for zero-latency:
//! 1. **Camera Thread**: Captures frames continuously from a Video/USB source.
//! 2. **AI Worker Thread**: Consumes the *latest* available frame (LIFO),
//!    performs inference, and yields results via an asynchronous channel.
//!
//! ## Quick Start
//! ```no_run
//! use ctrl_eqip::prelude::*;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // 1. Configure the AI Engine
//!     let config = EngineConfig::new("models/yolov8n.onnx", (640, 640), CameraConfig::default());
//!     
//!     // 2. Start the Pipeline
//!     let mut handle = DetectorPipeline::start(config)?;
//!     let analyzer = SceneAnalyzer::new(640);
//!     let estimator = DistanceEstimator::new(640);
//!
//!     // 3. Process results in a loop
//!     loop {
//!         if let Some(res) = handle.try_recv() {
//!             let tracking = analyzer.analyze(&res.detection.boxes, &estimator, 0);
//!             if tracking.has_person {
//!                 println!("Detected {} person(s). Closest: {:.2}m", 
//!                     tracking.person_count, 
//!                     tracking.closest_distance_m.unwrap_or(0.0));
//!             }
//!         }
//!     }
//! }
//! ```

pub mod engine;
pub mod logic;
pub mod protocol;
pub mod transport;

/// Re-exports phổ biến để sử dụng nhanh
pub mod prelude {
    pub use crate::engine::{
        DetectorPipeline, EngineConfig, EngineHandle, PipelineResult, 
        DetectorConfig, DetectionResult, BoundingBox, CameraConfig
    };
    pub use crate::logic::{
        SceneAnalyzer, TrackingResult, PersonDetail, 
        PresenceTracker, DistanceEstimator, DistanceCategory
    };
    pub use crate::protocol::messages::Message;
    pub use crate::transport::{SerialTransport, Transport};
}

// Re-export một số struct quan trọng ra level cao nhất cho tiện
pub use engine::{DetectorPipeline, EngineConfig, EngineHandle, PipelineResult};
pub use logic::{SceneAnalyzer, TrackingResult, DistanceEstimator};
