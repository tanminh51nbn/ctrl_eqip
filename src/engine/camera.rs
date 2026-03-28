//! # Camera Module
//!
//! Provides abstractions for capturing RGB frames from USB or RTSP sources.
//! This module is designed to be backend-agnostic for future expansion.

use std::time::Instant;

use thiserror::Error;

use nokhwa::{
    pixel_format::RgbFormat,
    utils::{
        CameraFormat, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType, Resolution,
    },
    Camera,
};

/// Configuration parameters for camera initialization.
#[derive(Debug, Clone)]
pub struct CameraConfig {
    /// Optional camera source string (e.g., "0" or "rtsp://...").
    /// If `None`, the `index` field will be used.
    pub source: Option<String>,
    /// USB camera index (0 = first detected camera).
    pub index: u32,
    /// Targeted capture resolution width.
    pub width: u32,
    /// Targeted capture resolution height.
    pub height: u32,
    /// Targeted frames per second (FPS).
    pub fps: u32,
    /// Flip frame horizontally (mirror effect) for easier monitoring.
    pub flip_horizontal: bool,
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self {
            source: None,
            index: 0,
            width: 640,
            height: 480,
            fps: 30,
            flip_horizontal: false,
        }
    }
}

/// A raw RGB frame captured from a camera.
#[derive(Debug, Clone)]
pub struct CameraFrame {
    /// Flattened RGB pixel buffer (row-major, 3 bytes per pixel).
    pub rgb: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Precise timestamp when the frame was captured.
    pub captured_at: Instant,
}

/// Errors related to hardware camera access and frame capture.
#[derive(Debug, Error)]
pub enum CameraError {
    #[error("failed to open camera: {0}")]
    OpenFailed(String),
    #[error("frame capture failed: {0}")]
    CaptureFailed(String),
    #[error("RGB buffer size mismatch: expected {expected}, got {got}")]
    BufferSizeMismatch { expected: usize, got: usize },
    #[error("RTSP support is disabled (requires 'opencv' feature)")]
    RtspDisabled,
}

/// Generic interface for any video source.
pub trait VideoProvider {
    /// Captures a single frame from the source.
    fn capture_frame(&mut self) -> Result<CameraFrame, CameraError>;
}

/// Factory function to create a camera provider based on the provided configuration.
/// - If `source` starts with `rtsp://` or `http://`, an RTSP backend is used (requires `opencv` feature).
/// - If `source` is a numeric string, it is treated as a USB device index.
/// - Otherwise, defaults to a standard USB camera using the `index` field.
pub fn create_camera(config: &CameraConfig) -> Result<Box<dyn VideoProvider>, CameraError> {
    if let Some(source) = config.source.as_deref() {
        if source.starts_with("rtsp://") || source.starts_with("http://") {
            #[cfg(feature = "opencv")]
            {
                return Ok(Box::new(RtspCamera::new(source, config.flip_horizontal)?));
            }
            #[cfg(not(feature = "opencv"))]
            {
                return Err(CameraError::RtspDisabled);
            }
        }

        if let Ok(index) = source.parse::<u32>() {
            return Ok(Box::new(UsbCamera::new_with_index(config, index)?));
        }
    }

    Ok(Box::new(UsbCamera::new(config)?))
}

/// Standard USB camera implementation using the `nokhwa` crate.
pub struct UsbCamera {
    cam: Camera,
    width: u32,
    height: u32,
    flip_horizontal: bool,
}

impl UsbCamera {
    /// Initializes a USB camera based on global config.
    pub fn new(config: &CameraConfig) -> Result<Self, CameraError> {
        Self::new_with_index(config, config.index)
    }

    /// Initializes a USB camera with a specific hardware index.
    pub fn new_with_index(config: &CameraConfig, index: u32) -> Result<Self, CameraError> {
        let index = CameraIndex::Index(index);
        let frame_formats = [FrameFormat::MJPEG, FrameFormat::YUYV];

        for frame_format in frame_formats {
            let target_format = CameraFormat::new(
                Resolution::new(config.width, config.height),
                frame_format,
                config.fps,
            );
            let format =
                RequestedFormat::new::<RgbFormat>(RequestedFormatType::Closest(target_format));

            if let Ok(mut cam) = Camera::new(index.clone(), format) {
                if cam.open_stream().is_ok() {
                    let res = cam.camera_format().resolution();
                    let width = res.width();
                    let height = res.height();
                    log::info!(
                        "[UsbCamera] format={:?} {}x{} @ {}fps",
                        frame_format,
                        width,
                        height,
                        config.fps
                    );

                    return Ok(Self {
                        cam,
                        width,
                        height,
                        flip_horizontal: config.flip_horizontal,
                    });
                }
            }
        }

        Err(CameraError::OpenFailed(
            "Failed to initialize USB camera (all MJPEG/YUYV attempts failed)".to_string(),
        ))
    }

    /// Returns the actual resolution after camera initialization.
    pub fn resolution(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

impl VideoProvider for UsbCamera {
    fn capture_frame(&mut self) -> Result<CameraFrame, CameraError> {
        let frame = self
            .cam
            .frame()
            .map_err(|e| CameraError::CaptureFailed(format!("camera.frame(): {}", e)))?;

        let rgb_image = frame
            .decode_image::<RgbFormat>()
            .map_err(|e| CameraError::CaptureFailed(format!("decode_image: {}", e)))?;

        let mut raw = rgb_image.into_raw();
        let expected = (self.width * self.height * 3) as usize;
        if raw.len() != expected {
            return Err(CameraError::BufferSizeMismatch {
                expected,
                got: raw.len(),
            });
        }

        if self.flip_horizontal {
            flip_horizontal_rgb_in_place(&mut raw, self.width, self.height);
        }

        Ok(CameraFrame {
            rgb: raw,
            width: self.width,
            height: self.height,
            captured_at: Instant::now(),
        })
    }
}

#[cfg(feature = "opencv")]
pub struct RtspCamera {
    cap: opencv::videoio::VideoCapture,
    flip_horizontal: bool,
}

#[cfg(feature = "opencv")]
impl RtspCamera {
    pub fn new(url: &str, flip_horizontal: bool) -> Result<Self, CameraError> {
        use opencv::videoio::{VideoCapture, CAP_ANY};
        let mut cap = VideoCapture::from_file(url, CAP_ANY)
            .map_err(|e| CameraError::OpenFailed(format!("RTSP open error: {}", e)))?;
        let opened = cap
            .is_opened()
            .map_err(|e| CameraError::OpenFailed(format!("RTSP status check error: {}", e)))?;
        if !opened {
            return Err(CameraError::OpenFailed(
                "Failed to open RTSP stream (source unreachable)".to_string(),
            ));
        }
        Ok(Self {
            cap,
            flip_horizontal,
        })
    }
}

#[cfg(feature = "opencv")]
impl VideoProvider for RtspCamera {
    fn capture_frame(&mut self) -> Result<CameraFrame, CameraError> {
        use opencv::prelude::*;
        let mut frame = opencv::core::Mat::default();
        self.cap
            .read(&mut frame)
            .map_err(|e| CameraError::CaptureFailed(format!("RTSP read error: {}", e)))?;
        if frame.empty() {
            return Err(CameraError::CaptureFailed("Captured empty RTSP frame".to_string()));
        }

        let mut rgb = opencv::core::Mat::default();
        opencv::imgproc::cvt_color(&frame, &mut rgb, opencv::imgproc::COLOR_BGR2RGB, 0)
            .map_err(|e| CameraError::CaptureFailed(format!("In-flight BGR->RGB conversion failed: {}", e)))?;

        let size = rgb
            .size()
            .map_err(|e| CameraError::CaptureFailed(format!("Failed to retrieve RTSP frame dimensions: {}", e)))?;
        let width = size.width as u32;
        let height = size.height as u32;

        let mut data = rgb
            .data_bytes()
            .map_err(|e| CameraError::CaptureFailed(format!("Failed to access raw RTSP buffer: {}", e)))?
            .to_vec();

        let expected = (width * height * 3) as usize;
        if data.len() != expected {
            return Err(CameraError::BufferSizeMismatch {
                expected,
                got: data.len(),
            });
        }

        if self.flip_horizontal {
            flip_horizontal_rgb_in_place(&mut data, width, height);
        }

        Ok(CameraFrame {
            rgb: data,
            width,
            height,
            captured_at: Instant::now(),
        })
    }
}

/// Efficient in-place horizontal flip for 24-bit RGB pixel data.
fn flip_horizontal_rgb_in_place(rgb: &mut [u8], width: u32, height: u32) {
    let row_bytes = width as usize * 3;
    let width_usize = width as usize;

    for y in 0..height as usize {
        let row_start = y * row_bytes;
        let row = &mut rgb[row_start..row_start + row_bytes];
        for x in 0..(width_usize / 2) {
            let left = x * 3;
            let right = (width_usize - 1 - x) * 3;
            row.swap(left, right);
            row.swap(left + 1, right + 1);
            row.swap(left + 2, right + 2);
        }
    }
}
