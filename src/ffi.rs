//! # FFI Bridge (C-API)
//!
//! Provides a stable C-compatible interface for calling `ctrl_eqip` from C or C++.
//! This module uses opaque pointers to manage Rust objects in C memory.

#![allow(unsafe_attributes)]

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::ptr;

use crate::prelude::*;

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
///
/// # Arguments
/// * `model_path` - Null-terminated string to the ONNX model file.
/// * `width` - Camera capture width (e.g. 640).
/// * `height` - Camera capture height (e.g. 480).
///
/// Returns a `CePipelineHandle` on success, or `NULL` on failure.
#[no_mangle]
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
///
/// # Returns
/// `1` if a new result was available and `out_result` was updated.
/// `0` if no new data was available.
#[no_mangle]
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
#[no_mangle]
pub unsafe extern "C" fn ce_pipeline_stop(handle: CePipelineHandle) {
    if !handle.is_null() {
        let _ = unsafe { Box::from_raw(handle as *mut EngineHandle) };
        // Dropping the handle shuts down the spawned threads
        log::info!("[FFI] Pipeline stopped and handle released.");
    }
}

/// Helper to get the last error (placeholder for more robust error handling).
#[no_mangle]
pub unsafe extern "C" fn ce_get_version() -> *mut c_char {
    let s = CString::new("0.1.0 (ctrl_eqip)").unwrap();
    s.into_raw()
}

/// Free strings allocated by Rust (like version string).
#[no_mangle]
pub unsafe extern "C" fn ce_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe { let _ = CString::from_raw(s); }
    }
}
