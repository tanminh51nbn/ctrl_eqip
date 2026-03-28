//! # Engine Module
//!
//! Provides the core AI and Camera infrastructure:
//! - Camera abstraction for USB/RTSP sources.
//! - ONNX Runtime inference engine for body detection.
//! - Image preprocessing (Letterboxing) and post-processing (YOLO decoding).

pub mod camera;
pub mod detector;
pub mod engine;
pub mod processing;

pub use camera::{create_camera, CameraConfig, CameraError, CameraFrame, UsbCamera, VideoProvider};
pub use detector::{
    BodyDetector, BoundingBox, DetectionResult, DetectorConfig, DetectorError, DetectorPipeline,
    EngineConfig, EngineHandle, PipelineResult, OutputFormat, PipelineError,
};
