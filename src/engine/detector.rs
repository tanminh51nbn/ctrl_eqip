//! # Body Detector (Edge Inference Library)
//!
//! This module wraps an ONNX model for human detection and provides a clean API for inference.
//! Optimized for Edge AI applications where data is sent to microcontrollers (ESP32/STM32):
//! 1. Minimal hardware acceleration dependencies, favoring predictable CPU performance.
//! 2. Flexible input configuration (Model size, Camera source).
//! 3. Implements a LIFO Producer-Consumer Pattern using dual threads (Camera & AI).

use ndarray::ArrayView3;
use ort::session::builder::GraphOptimizationLevel;
use ort::{inputs, session::Session};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;

use super::camera::{create_camera, CameraConfig, CameraError, CameraFrame};

// ─── Config ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DetectorConfig {
    pub input_size: (u32, u32),
    pub conf_threshold: f32,
    pub nms_iou_threshold: f32,
    pub person_class_id: usize,
    /// Bounding box output format from the model.
    pub output_format: OutputFormat,
}

impl Default for DetectorConfig {
    fn default() -> Self {
        Self {
            input_size: (640, 640),
            conf_threshold: 0.50,
            nms_iou_threshold: 0.45,
            person_class_id: 0,
            output_format: OutputFormat::Auto,
        }
    }
}

/// Bounding box output formats supported by the decoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Auto,
    CxCyWhNormalized,
    CxCyWhPixels,
    XyXyNormalized,
    XyXyPixels,
}

/// Integrated configuration for the pipeline (camera + detector).
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Path to the ONNX model file.
    pub model_path: String,
    /// Detector-specific settings (thresholds, NMS, etc.).
    pub detector: DetectorConfig,
    /// Camera hardware settings.
    pub camera: CameraConfig,
    /// Target capture rate for the camera (default: 24.0).
    pub target_camera_fps: f64,
}

impl EngineConfig {
    pub fn new(
        model_path: impl Into<String>,
        input_size: (u32, u32),
        camera: CameraConfig,
    ) -> Self {
        let mut detector = DetectorConfig::default();
        detector.input_size = input_size;
        Self {
            model_path: model_path.into(),
            detector,
            camera,
            target_camera_fps: 24.0,
        }
    }
}

// ─── Output types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct BoundingBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub confidence: f32,
    /// Class index from the AI model.
    pub class_id: usize,
}

impl BoundingBox {
    pub fn area(&self) -> f32 {
        self.width * self.height
    }

    pub fn iou_with(&self, other: &BoundingBox) -> f32 {
        let inter_x1 = self.x.max(other.x);
        let inter_y1 = self.y.max(other.y);
        let inter_x2 = (self.x + self.width).min(other.x + other.width);
        let inter_y2 = (self.y + self.height).min(other.y + other.height);
        let inter_w = (inter_x2 - inter_x1).max(0.0);
        let inter_h = (inter_y2 - inter_y1).max(0.0);
        let intersection = inter_w * inter_h;
        let union = self.area() + other.area() - intersection;
        if union <= 0.0 {
            0.0
        } else {
            intersection / union
        }
    }
}

#[derive(Debug, Clone)]
pub struct FrameTimings {
    pub preprocess_ms: f64,
    pub inference_ms: f64,
    pub postprocess_ms: f64,
    pub total_ms: f64,
}

#[derive(Debug, Clone)]
pub struct DetectionResult {
    /// Indicates if at least one person was detected.
    pub has_person: bool,
    /// List of all detected bounding boxes and class metadata.
    pub boxes: Vec<BoundingBox>,
    pub frame_width: u32,
    pub frame_height: u32,
    pub camera_fps: f32,
    pub timings: FrameTimings,
}

/// Result yielded from the AI pipeline back to the main loop.
pub struct PipelineResult {
    pub detection: DetectionResult,
    pub captured_at: Instant,
    pub frame: Arc<CameraFrame>,
}

// ─── Errors ──────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum DetectorError {
    #[error("failed to load model from '{path}': {reason}")]
    LoadFailed { path: String, reason: String },

    #[error("ONNX Runtime error: {0}")]
    OrtError(#[from] ort::Error),

    #[error("invalid frame resolution: {width}x{height}")]
    InvalidFrame { width: u32, height: u32 },

    #[error("post-processing error: {0}")]
    PostprocessError(String),
}

/// Pipeline errors involving camera capture or inference execution.
#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("camera error: {0}")]
    Camera(#[from] CameraError),

    #[error("detector error: {0}")]
    Detector(#[from] DetectorError),

    #[error("failed to spawn thread: {0}")]
    Thread(String),
}

// ─── BodyDetector (AI Logic) ────────────────────────────────────────────────

pub struct BodyDetector {
    config: DetectorConfig,
    session: Session,
}

impl BodyDetector {
    pub fn load_with_config(
        model_path: impl Into<String>,
        config: DetectorConfig,
    ) -> Result<Self, DetectorError> {
        let path = model_path.into();

        if !std::path::Path::new(&path).exists() {
            return Err(DetectorError::LoadFailed {
                path: path.clone(),
                reason: "file does not exist".to_string(),
            });
        }

        // Initialize session with CPU optimization (favors stability on edge nodes)
        let mut builder = Session::builder()
            .map_err(|e| DetectorError::LoadFailed {
                path: path.clone(),
                reason: format!("SessionBuilder error: {}", e),
            })?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| DetectorError::LoadFailed {
                path: path.clone(),
                reason: format!("opt level error: {}", e),
            })?
            .with_intra_threads(2)
            .map_err(|e| DetectorError::LoadFailed {
                path: path.clone(),
                reason: format!("intra threads error: {}", e),
            })?;

        let model_bytes = std::fs::read(&path).map_err(|e| DetectorError::LoadFailed {
            path: path.clone(),
            reason: format!("failed to read model file: {}", e),
        })?;

        let session = builder
            .commit_from_memory(&model_bytes)
            .map_err(|e| DetectorError::LoadFailed {
                path: path.clone(),
                reason: format!("failed to load model into memory: {}", e),
            })?;

        log::info!(
            "[BodyDetector] Model initialized: '{}' | input={}x{} | conf={}",
            path,
            config.input_size.0,
            config.input_size.1,
            config.conf_threshold
        );

        Ok(Self {
            session,
            config,
        })
    }

    pub fn detect(
        &mut self,
        frame: &CameraFrame,
        camera_fps: f32,
    ) -> Result<DetectionResult, DetectorError> {
        let total_start = Instant::now();
        let preprocess_start = Instant::now();

        let frame_width = frame.width;
        let frame_height = frame.height;

        if frame_width == 0 || frame_height == 0 {
            return Err(DetectorError::InvalidFrame {
                width: frame_width,
                height: frame_height,
            });
        }

        let (in_w, in_h) = self.config.input_size;

        // 1. PREPROCESS (Apply Letterboxing to maintain aspect ratio)
        let preprocess_res = super::processing::preprocess(&frame.rgb, frame.width, frame.height, in_w, in_h);
        let tensor = preprocess_res.tensor;
        let preprocess_ms = preprocess_start.elapsed().as_secs_f64() * 1000.0;

        // 2. INFERENCE
        let inference_start = Instant::now();
        let input_ort = ort::value::Tensor::from_array(tensor).map_err(DetectorError::OrtError)?;

        let input_name = self.session.inputs()[0].name().to_string();
        let output_name = self.session.outputs()[0].name().to_string();

        let outputs = self.session.run(inputs![&*input_name => input_ort])?;
        let inference_ms = inference_start.elapsed().as_secs_f64() * 1000.0;

        // 3. POST-PROCESS (Decode bounding boxes)
        let postprocess_start = Instant::now();
        let (shape, raw_data) = outputs[&*output_name].try_extract_tensor::<f32>()?;

        let dims: Vec<usize> = shape.iter().map(|&d| d as usize).collect();
        if dims.len() != 3 {
            return Err(DetectorError::PostprocessError(format!(
                "Invalid output tensor dimensions: {:?}",
                dims
            )));
        }

        let output_view =
            ArrayView3::from_shape((dims[0], dims[1], dims[2]), raw_data).map_err(|_| {
                DetectorError::PostprocessError(
                    "Failed to map raw output to ArrayView3".to_string(),
                )
            })?;

        let boxes = super::processing::postprocess(
            &output_view,
            &preprocess_res.meta,
            self.config.conf_threshold,
            self.config.nms_iou_threshold,
        );
        
        let has_person = boxes.iter().any(|b| b.class_id == self.config.person_class_id);

        let postprocess_ms = postprocess_start.elapsed().as_secs_f64() * 1000.0;

        Ok(DetectionResult {
            has_person,
            boxes,
            frame_width,
            frame_height,
            camera_fps,
            timings: FrameTimings {
                preprocess_ms,
                inference_ms,
                postprocess_ms,
                total_ms: total_start.elapsed().as_secs_f64() * 1000.0,
            },
        })
    }


}

// ─── Pipeline (Camera -> AI -> Main) ──────────────────────────────────────────

pub struct EngineHandle {
    running: Arc<AtomicBool>,
    rx: Receiver<PipelineResult>,
    camera_join: Option<thread::JoinHandle<()>>,
    infer_join: Option<thread::JoinHandle<()>>,
}

impl EngineHandle {
    pub fn try_recv(&self) -> Option<PipelineResult> {
        self.rx.try_recv().ok()
    }
    
    pub fn recv_timeout(&self, timeout: Duration) -> Option<PipelineResult> {
        self.rx.recv_timeout(timeout).ok()
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.camera_join.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.infer_join.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for EngineHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

pub struct DetectorPipeline;

impl DetectorPipeline {
    /// Starts the high-performance LIFO multi-threaded pipeline.
    pub fn start(config: EngineConfig) -> Result<EngineHandle, PipelineError> {
        let running = Arc::new(AtomicBool::new(true));

        // Shared latest frame state (LIFO buffer)
        let latest_frame: Arc<Mutex<Option<(Arc<CameraFrame>, f32)>>> = Arc::new(Mutex::new(None));
        
        let (notify_tx, notify_rx) = mpsc::sync_channel::<()>(1);
        let (output_tx, output_rx) = mpsc::sync_channel::<PipelineResult>(5);
        let (init_tx, init_rx) = mpsc::sync_channel::<Result<(), DetectorError>>(1);

        // 1. Start AI Worker Thread
        let infer_running = Arc::clone(&running);
        let infer_frame = Arc::clone(&latest_frame);
        let model_path = config.model_path.clone();
        let detector_cfg = config.detector.clone();

        let infer_join = thread::Builder::new()
            .name("eqip-ai-worker".to_string())
            .spawn(move || {
                let mut detector = match BodyDetector::load_with_config(&model_path, detector_cfg) {
                    Ok(d) => {
                        let _ = init_tx.send(Ok(()));
                        d
                    }
                    Err(e) => {
                        let _ = init_tx.send(Err(e));
                        return;
                    }
                };

                while infer_running.load(Ordering::Relaxed) {
                    // Wait for wakeup signal from the Camera thread
                    if let Ok(_) = notify_rx.recv() {
                        let packet_opt = infer_frame.lock().unwrap().take();
                        if let Some((packet, cam_fps)) = packet_opt {
                            if let Ok(detection) = detector.detect(packet.as_ref(), cam_fps) {
                                let result = PipelineResult {
                                    detection,
                                    captured_at: packet.captured_at,
                                    frame: packet,
                                };
                                let _ = output_tx.try_send(result);
                            }
                        }
                    }
                }
            })
            .map_err(|e| PipelineError::Thread(e.to_string()))?;

        // Checking AI initialization
        if let Ok(Err(err)) = init_rx.recv_timeout(Duration::from_secs(10)) {
            running.store(false, Ordering::Relaxed);
            let _ = infer_join.join();
            return Err(PipelineError::Detector(err));
        }

        let (cam_init_tx, cam_init_rx) = mpsc::sync_channel::<Result<(), CameraError>>(1);

        // 2. Start Camera Thread (Producer)
        let camera_running = Arc::clone(&running);
        let camera_frame = Arc::clone(&latest_frame);
        let camera_cfg = config.camera.clone();
        let target_frame_ms = 1000.0 / config.target_camera_fps;

        let camera_join = thread::Builder::new()
            .name("eqip-camera".to_string())
            .spawn(move || {
                let mut edge_cam = match create_camera(&camera_cfg) {
                    Ok(cam) => {
                        let _ = cam_init_tx.send(Ok(()));
                        cam
                    }
                    Err(e) => {
                        let _ = cam_init_tx.send(Err(e));
                        return;
                    }
                };

                let mut pacer = Instant::now();
                let mut last_fps_check = Instant::now();
                let mut frame_count = 0;
                let mut frame_index = 0u64;
                let mut current_cam_fps = 0.0;

                while camera_running.load(Ordering::Relaxed) {
                    if let Ok(raw_frame) = edge_cam.capture_frame() {
                        frame_count += 1;
                        if last_fps_check.elapsed().as_secs() >= 1 {
                            current_cam_fps = frame_count as f32 / last_fps_check.elapsed().as_secs_f32();
                            frame_count = 0;
                            last_fps_check = Instant::now();
                        }

                        let shared_image = Arc::new(raw_frame);

                        // Update latest frame + current capture FPS
                        *camera_frame.lock().unwrap() = Some((shared_image, current_cam_fps));

                        // Wake up AI Worker, throttled to every 3rd frame to avoid congestion
                        frame_index += 1;
                        if frame_index % 3 == 0 { 
                            let _ = notify_tx.try_send(());
                        }

                        let elapsed_ms = pacer.elapsed().as_secs_f64() * 1000.0;
                        if elapsed_ms < target_frame_ms {
                            let sleep_time = target_frame_ms - elapsed_ms;
                            thread::sleep(Duration::from_millis(sleep_time as u64));
                        }
                        pacer = Instant::now();
                    } else {
                        // Sleep to allow camera recovery
                        thread::sleep(Duration::from_millis(50));
                    }
                }
            })
            .map_err(|e| PipelineError::Thread(e.to_string()))?;

        // Checking Camera initialization
        if let Ok(Err(err)) = cam_init_rx.recv_timeout(Duration::from_secs(10)) {
            running.store(false, Ordering::Relaxed);
            let _ = infer_join.join();
            let _ = camera_join.join();
            return Err(PipelineError::Camera(err));
        }

        Ok(EngineHandle {
            running,
            rx: output_rx,
            camera_join: Some(camera_join),
            infer_join: Some(infer_join),
        })
    }
}
