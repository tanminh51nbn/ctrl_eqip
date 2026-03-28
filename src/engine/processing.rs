use fast_image_resize as fir;
use ndarray::{Array4, ArrayView3};
use rayon::prelude::*;
use std::num::NonZeroU32;

use crate::engine::detector::BoundingBox;

/// Metadata for reversing the Letterbox transformation.
pub struct LetterboxMeta {
    /// Scaling factor applied to the original image.
    pub scale: f32,
    /// Horizontal padding (pixels) added to the left/right.
    pub pad_w: u32,
    /// Vertical padding (pixels) added to the top/bottom.
    pub pad_h: u32,
}

/// Result of the preprocessing step, ready for AI inference.
pub struct PreprocessOutput {
    /// NCHW tensor (float32, normalized to 0.0-1.0).
    pub tensor: Array4<f32>,
    /// Metadata required to map output coordinates back to the original image space.
    pub meta: LetterboxMeta,
}

/// Prepares a raw RGB buffer for YOLOv8/v11 inference using Letterbox scaling.
///
/// 1. Resizes while maintaining aspect ratio (using Catmull-Rom convolution).
/// 2. Pads the remaining area with a neutral gray (114) to reach the target size.
/// 3. Normalizes pixel values to [0.0, 1.0] and rearranges to NCHW format.
pub fn preprocess(
    rgb: &[u8],
    orig_w_u32: u32,
    orig_h_u32: u32,
    input_w: u32,
    input_h: u32,
) -> PreprocessOutput {
    let orig_w = orig_w_u32 as f32;
    let orig_h = orig_h_u32 as f32;
    let target_w = input_w as f32;
    let target_h = input_h as f32;
    let scale = (target_w / orig_w).min(target_h / orig_h);

    let new_unpad_w = (orig_w * scale).round() as u32;
    let new_unpad_h = (orig_h * scale).round() as u32;

    let src_image = fir::Image::from_vec_u8(
        NonZeroU32::new(orig_w_u32).expect("Invalid source image width"),
        NonZeroU32::new(orig_h_u32).expect("Invalid source image height"),
        rgb.to_vec(),
        fir::PixelType::U8x3,
    )
    .expect("Failed to initialize source image buffer");

    let mut dst_image = fir::Image::new(
        NonZeroU32::new(new_unpad_w).expect("Invalid resized image width"),
        NonZeroU32::new(new_unpad_h).expect("Invalid resized image height"),
        fir::PixelType::U8x3,
    );

    let mut resizer = fir::Resizer::new(fir::ResizeAlg::Convolution(fir::FilterType::CatmullRom));
    resizer
        .resize(&src_image.view(), &mut dst_image.view_mut())
        .expect("Image resize operation failed");
    let resized_data = dst_image.buffer();

    let pad_w = (input_w - new_unpad_w) / 2;
    let pad_h = (input_h - new_unpad_h) / 2;

    let mut input_tensor =
        Array4::<f32>::from_elem((1, 3, input_h as usize, input_w as usize), 114.0 / 255.0);

    let resized_w = new_unpad_w as usize;
    let resized_h = new_unpad_h as usize;
    let input_w_usize = input_w as usize;
    let input_h_usize = input_h as usize;
    let pad_w_usize = pad_w as usize;
    let pad_h_usize = pad_h as usize;
    let tensor_slice = input_tensor
        .as_slice_mut()
        .expect("Tensor memory must be contiguous");
    let hw = input_h_usize * input_w_usize;
    let data = resized_data;
    let row_stride = resized_w * 3;

    let (r_channel, rest) = tensor_slice.split_at_mut(hw);
    let (g_channel, b_channel) = rest.split_at_mut(hw);
    let valid_y_start = pad_h_usize;
    let valid_y_end = pad_h_usize + resized_h;

    r_channel
        .par_chunks_mut(input_w_usize)
        .zip(g_channel.par_chunks_mut(input_w_usize))
        .zip(b_channel.par_chunks_mut(input_w_usize))
        .enumerate()
        .for_each(|(y, ((r_row, g_row), b_row))| {
            if y < valid_y_start || y >= valid_y_end {
                return;
            }
            let src_y = y - valid_y_start;
            let src_row = &data[src_y * row_stride..(src_y + 1) * row_stride];
            for x in 0..resized_w {
                let src_idx = x * 3;
                let dst_idx = pad_w_usize + x;
                r_row[dst_idx] = src_row[src_idx] as f32 / 255.0;
                g_row[dst_idx] = src_row[src_idx + 1] as f32 / 255.0;
                b_row[dst_idx] = src_row[src_idx + 2] as f32 / 255.0;
            }
        });

    PreprocessOutput {
        tensor: input_tensor,
        meta: LetterboxMeta {
            scale,
            pad_w,
            pad_h,
        },
    }
}

/// Decodes YOLOv8/v11 raw tensor output into physical bounding boxes.
///
/// Performs coordinate transformation (Inverse Letterbox) and dynamic class scoring.
pub fn postprocess(
    output_view: &ArrayView3<f32>,
    meta: &LetterboxMeta,
    conf_threshold: f32,
    iou_threshold: f32,
) -> Vec<BoundingBox> {
    let mut boxes = Vec::new();

    // YOLOv8 output: [batch=1, features=4+classes, anchors=Variable]
    // Example: [1, 5, 5040] for 1 class (e.g. Person)
    let shape = output_view.shape();
    let num_features = shape[1];
    let num_anchors = shape[2];

    for i in 0..num_anchors {
        // Find class with highest score (starts from index 4)
        let mut max_score = 0.0;
        let mut class_id = 0;

        for c in 4..num_features {
            let score = output_view[[0, c, i]];
            if score > max_score {
                max_score = score;
                class_id = c - 4;
            }
        }

        if max_score > conf_threshold {
            let cx = output_view[[0, 0, i]];
            let cy = output_view[[0, 1, i]];
            let w = output_view[[0, 2, i]];
            let h = output_view[[0, 3, i]];

            // YOLOv8/v11 usually returns coordinates relative to input size (e.g. 0-640).
            // We assume standard YOLOv8 Pixel format here.
            let real_cx = (cx - meta.pad_w as f32) / meta.scale;
            let real_cy = (cy - meta.pad_h as f32) / meta.scale;
            let real_w = w / meta.scale;
            let real_h = h / meta.scale;

            boxes.push(BoundingBox {
                x: real_cx - real_w / 2.0,
                y: real_cy - real_h / 2.0,
                width: real_w,
                height: real_h,
                class_id,
                confidence: max_score,
            });
        }
    }

    apply_nms(boxes, iou_threshold)
}

/// Standard Non-Maximum Suppression (NMS) to eliminate overlapping detections.
fn apply_nms(mut boxes: Vec<BoundingBox>, iou_threshold: f32) -> Vec<BoundingBox> {
    boxes.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut selected_boxes: Vec<BoundingBox> = Vec::new();

    for current_box in boxes {
        let mut keep = true;

        for selected_box in &selected_boxes {
            if current_box.class_id != selected_box.class_id {
                continue;
            }

            let iou = calculate_iou(&current_box, selected_box);

            if iou > iou_threshold {
                keep = false;
                break;
            }
        }

        if keep {
            selected_boxes.push(current_box);
        }
    }

    selected_boxes
}

/// Calculates Intersection over Union (IoU) between two bounding boxes.
fn calculate_iou(box1: &BoundingBox, box2: &BoundingBox) -> f32 {
    let x1 = box1.x.max(box2.x);
    let y1 = box1.y.max(box2.y);
    let x2 = (box1.x + box1.width).min(box2.x + box2.width);
    let y2 = (box1.y + box1.height).min(box2.y + box2.height);

    let intersection_area = (x2 - x1).max(0.0) * (y2 - y1).max(0.0);

    let box1_area = box1.width * box1.height;
    let box2_area = box2.width * box2.height;

    let union_area = box1_area + box2_area - intersection_area;

    if union_area > 0.0 {
        intersection_area / union_area
    } else {
        0.0
    }
}
