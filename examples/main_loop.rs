use ctrl_eqip::prelude::*;
use minifb::{Key, Window, WindowOptions};
use std::time::{Duration, Instant};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Testing ctrl_eqip Library with Visualization ---");
    
    // 1. Cấu hình Engine & Camera
    let mut camera_cfg = CameraConfig::default();
    camera_cfg.flip_horizontal = true; // Sửa lỗi bị ngược mirror

    let mut engine_cfg = EngineConfig::new(
        "models/Body-Detection-Model_640x384.onnx", 
        (640, 384), 
        camera_cfg
    );
    engine_cfg.detector.conf_threshold = 0.45;
    engine_cfg.detector.nms_iou_threshold = 0.45;

    // 2. Khởi tạo Logic
    let analyzer = SceneAnalyzer::new(480);
    let estimator = DistanceEstimator::new(480);

    // 3. Chuẩn bị Window để hiển thị
    let mut window = Window::new(
        "ctrl_eqip - AI Body Detection",
        640, // Width
        480, // Height
        WindowOptions::default(),
    ).expect("Không thể tạo cửa sổ hiển thị");

    // Giới hạn FPS của Window (không phải của AI)
    window.limit_update_rate(Some(std::time::Duration::from_micros(16600))); // ~60fps

    // 4. Chạy Pipeline (2 luồng ngầm)
    println!("[Main] Đang khởi động Pipeline...");
    let handle = DetectorPipeline::start(engine_cfg)?;
    println!("[Main] Pipeline đang chạy. Nhấn ESC trong cửa sổ hoặc Ctrl+C để dừng.");

    // 5. Vòng lặp quan sát 
    let mut last_log_time = Instant::now();
    let log_interval = Duration::from_secs(3);
    let mut frames_processed = 0;
    let mut last_tracking: Option<TrackingResult> = None;
    let mut last_camera_fps = 0.0f32;

    // Xử lý dừng
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        println!("\n[Main] Đang dừng...");
        r.store(false, Ordering::SeqCst);
    })?;

    // Buffer để hiển thị (format u32 ARGB cho minifb)
    let mut display_buffer: Vec<u32> = vec![0; 640 * 480];

    while running.load(Ordering::SeqCst) && window.is_open() && !window.is_key_down(Key::Escape) {
        // Lấy kết quả mới nhất từ Pipeline (LIFO)
        if let Some(pipeline_res) = handle.try_recv() {
            frames_processed += 1;
            last_camera_fps = pipeline_res.detection.camera_fps;
            
            // AI Detections -> Logic Tracking
            let tracking = analyzer.analyze(&pipeline_res.detection.boxes, &estimator, 0);
            
            // Cập nhật frame vào buffer hiển thị
            let frame = &pipeline_res.frame;
            if display_buffer.len() != (frame.width * frame.height) as usize {
                display_buffer.resize((frame.width * frame.height) as usize, 0);
            }

            // Convert RGB (u8x3) -> ARGB (u32)
            for i in 0..(frame.width * frame.height) as usize {
                let r = frame.rgb[i * 3] as u32;
                let g = frame.rgb[i * 3 + 1] as u32;
                let b = frame.rgb[i * 3 + 2] as u32;
                display_buffer[i] = (r << 16) | (g << 8) | b;
            }

            // Vẽ Bounding Boxes đơn giản (không cần ImageProc cho nhanh)
            for bbox in &pipeline_res.detection.boxes {
                draw_rect(&mut display_buffer, frame.width as usize, frame.height as usize, bbox, 0x00FF00); // Màu xanh lá
            }

            last_tracking = Some(tracking);
            
            // Cập nhật cửa sổ
            window.update_with_buffer(&display_buffer, frame.width as usize, frame.height as usize).unwrap();
        } else {
            // Nếu không có frame mới, vẫn cần gọi window.update để duy trì cửa sổ (tránh treo)
            window.update();
        }

        // In log mỗi 3 giây
        if last_log_time.elapsed() >= log_interval {
            println!("--- Thống kê 3s gần nhất ---");
            println!("FPS AI    : {:.1}", frames_processed as f32 / 3.0);
            println!("FPS Camera: {:.1}", last_camera_fps);
            if let Some(res) = &last_tracking {
                println!("Người     : {} | Gần nhất: {:.2}m", res.person_count, res.closest_distance_m.unwrap_or(0.0));
            } else {
                println!("Người     : 0");
            }
            println!("---------------------------\n");
            
            frames_processed = 0;
            last_log_time = Instant::now();
        }

        std::thread::sleep(Duration::from_millis(1));
    }

    println!("[Main] Đã dừng. Tạm biệt!");
    Ok(())
}

/// Hàm vẽ khung chữ nhật đơn giản lên buffer u32
fn draw_rect(buffer: &mut [u32], w: usize, h: usize, bbox: &BoundingBox, color: u32) {
    let x1 = (bbox.x.max(0.0) as usize).min(w - 1);
    let y1 = (bbox.y.max(0.0) as usize).min(h - 1);
    let x2 = ((bbox.x + bbox.width).max(0.0) as usize).min(w - 1);
    let y2 = ((bbox.y + bbox.height).max(0.0) as usize).min(h - 1);

    // Cạnh ngang
    for x in x1..=x2 {
        buffer[y1 * w + x] = color;
        buffer[y2 * w + x] = color;
    }
    // Cạnh dọc
    for y in y1..=y2 {
        buffer[y * w + x1] = color;
        buffer[y * w + x2] = color;
    }
}