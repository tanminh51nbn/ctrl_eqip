// File: src/engine.rs
use crate::engine::processing;
use crate::engine::detector::BoundingBox;
use image::{ImageBuffer, Rgb};
use ort::inputs;
use ort::session::builder::GraphOptimizationLevel; // Đã cập nhật đường dẫn API mới của ort 2.0
use ort::session::Session;
use ort::execution_providers::{CUDAExecutionProvider, CoreMLExecutionProvider, OpenVINOExecutionProvider, TensorRTExecutionProvider, DirectMLExecutionProvider};
use std::error::Error;
use std::time::Instant;

const INTRA_THREAD: usize = 1;

pub struct AIEngine {
    session: Session,
    input_w: u32,
    input_h: u32,
}

pub struct FrameTimings {
    pub preprocess_ms: f64,
    pub inference_ms: f64,
    pub postprocess_ms: f64,
    pub total_ms: f64,
}

pub struct FrameOutput {
    pub boxes: Vec<BoundingBox>,
    pub timings: FrameTimings,
}

impl AIEngine {
    // 1. NẠP MÔ HÌNH (Load Model)
    pub fn new(model_path: &str, input_w: u32, input_h: u32) -> Result<Self, Box<dyn Error>> {
        // Cấu hình ORT Session để tận dụng phần cứng (Hardware Acceleration)
        // Hệ thống sẽ tự động tìm kiếm và ưu tiên dồn tải vào GPU/NPU, nếu không có sẽ Fallback về CPU.
        let session = Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_intra_threads(INTRA_THREAD)?
            .with_execution_providers([
                TensorRTExecutionProvider::default().build(),
                CUDAExecutionProvider::default().build(),
                DirectMLExecutionProvider::default().build(),
                OpenVINOExecutionProvider::default().build(),
                CoreMLExecutionProvider::default().build(),
            ])?
            .commit_from_file(model_path)?;

        Ok(Self {
            session,
            input_w,
            input_h,
        })
    }

    // Hàm thực thi toàn bộ luồng từ ảnh thô -> Danh sách Khung nhận diện
    pub fn process_frame(
        &mut self,
        img: &ImageBuffer<Rgb<u8>, Vec<u8>>,
    ) -> Result<FrameOutput, Box<dyn Error>> {
        let total_start = Instant::now();
        let preprocess_start = Instant::now();
        let preprocessing = processing::preprocess(img.as_raw(), img.width(), img.height(), self.input_w, self.input_h);
        let preprocess_ms = preprocess_start.elapsed().as_secs_f64() * 1000.0;

        // 3. CHẠY SUY LUẬN (Inference)
        // [FIX CHÍNH] Tạo Tensor chuẩn của ort 2.0 một cách tường minh từ mảng ndarray
        let inference_start = Instant::now();
        let input_ort = ort::value::Tensor::from_array(preprocessing.tensor)?;

        // Chèn input_ort vào macro inputs! (macro trả về Vec, không dùng dấu `?` trong ruột macro)
        let outputs = self.session.run(inputs!["images" => input_ort])?;
        let inference_ms = inference_start.elapsed().as_secs_f64() * 1000.0;

        // Trích xuất kết quả từ cổng đầu ra "output0"
        // Ở bản ort 2.0, hàm này trả về Tuple: (Shape, Dữ liệu thô &[f32])
        let (shape, raw_data) = outputs["output0"].try_extract_tensor::<f32>()?;
        let dims: Vec<usize> = shape.iter().map(|dim| *dim as usize).collect();
        if dims.len() != 3 {
            return Err("Lỗi: Kích thước tensor đầu ra không hợp lệ".into());
        }
        let output_view = ndarray::ArrayView3::from_shape((dims[0], dims[1], dims[2]), raw_data)
            .map_err(|_| "Lỗi: Không thể ép kiểu dữ liệu đầu ra về đúng kích thước")?;

        // 4. HẬU XỬ LÝ (Post-processing)
        let postprocess_start = Instant::now();
        let final_boxes = processing::postprocess(&output_view, &preprocessing.meta, 0.3, 0.45);
        let postprocess_ms = postprocess_start.elapsed().as_secs_f64() * 1000.0;

        Ok(FrameOutput {
            boxes: final_boxes,
            timings: FrameTimings {
                preprocess_ms,
                inference_ms,
                postprocess_ms,
                total_ms: total_start.elapsed().as_secs_f64() * 1000.0,
            },
        })
    }
}
