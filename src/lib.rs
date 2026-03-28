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

/// Common re-exports for quick access.
pub mod prelude {
    pub use crate::engine::*;
    pub use crate::logic::*;
    pub use crate::protocol::messages::Message;
    pub use crate::transport::*;
}

// --- C-API (FFI) Bridge ---

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::ptr;

// Use internal modules for FFI implementation
use crate::engine::{CameraConfig, DetectorPipeline, EngineConfig, EngineHandle};
use crate::logic::{DistanceEstimator, SceneAnalyzer};

/// Opaque handle representing the AI Detection Pipeline.
pub type CePipelineHandle = *mut c_void;

/// C-compatible representation of a Bounding Box.
#[repr(C)]
pub struct CeBoundingBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub confidence: f32,
    pub class_id: u32,
} 

/// C-compatible tracking result for a single frame.
#[repr(C)]
pub struct CeTrackingResult {
    /// Total number of detected people.
    pub person_count: u32,
    /// Meters to the closest person (0.0 if not found).
    pub closest_distance_m: f32,
    /// Whether any person is present.
    pub has_person: bool,
}

/// Starts the AI detection pipeline.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ce_pipeline_start(
    model_path: *const c_char,
    width: u32,
    height: u32,
) -> CePipelineHandle {
    if model_path.is_null() {
        return ptr::null_mut();
    }

    let c_str = unsafe { CStr::from_ptr(model_path) };
    let path = match c_str.to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    let mut cam_cfg = CameraConfig::default();
    cam_cfg.width = width;
    cam_cfg.height = height;
    cam_cfg.flip_horizontal = true;

    // Default YOLOv8 sizes
    let engine_cfg = EngineConfig::new(path, (640, 384), cam_cfg);

    match DetectorPipeline::start(engine_cfg) {
        Ok(handle) => Box::into_raw(Box::new(handle)) as CePipelineHandle,
        Err(e) => {
            log::error!("[FFI] Failed to start pipeline: {}", e);
            ptr::null_mut()
        }
    }
}

/// Polls the latest detection result from the pipeline.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ce_pipeline_try_recv(
    handle: CePipelineHandle,
    out_result: *mut CeTrackingResult,
) -> i32 {
    if handle.is_null() || out_result.is_null() {
        return 0;
    }

    let pipeline = unsafe { &*(handle as *mut EngineHandle) };
    let analyzer = SceneAnalyzer::new(480);
    let estimator = DistanceEstimator::new(480);

    if let Some(res) = pipeline.try_recv() {
        let tracking = analyzer.analyze(&res.detection.boxes, &estimator, 0);
        
        unsafe {
            (*out_result).person_count = tracking.person_count as u32;
            (*out_result).closest_distance_m = tracking.closest_distance_m.unwrap_or(0.0);
            (*out_result).has_person = tracking.has_person;
        }
        return 1;
    }

    0
}

/// Stops the pipeline and frees memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ce_pipeline_stop(handle: CePipelineHandle) {
    if !handle.is_null() {
        let _ = unsafe { Box::from_raw(handle as *mut EngineHandle) };
        log::info!("[FFI] Pipeline stopped and handle released.");
    }
}

/// Helper to get the last error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ce_get_version() -> *mut c_char {
    let s = CString::new("0.1.0 (ctrl_eqip)").unwrap();
    s.into_raw()
}

/// Free strings allocated by Rust.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ce_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe { let _ = CString::from_raw(s); }
    }
}
