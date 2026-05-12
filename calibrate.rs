use ctrl_eqip::prelude::*;
use minifb::{Key, Window, WindowOptions};
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("==================================================");
    println!("   CONG CU DO KHOANG CACH (CALIBRATION TOOL)  ");
    println!("==================================================");
    println!("Huong dan:");
    println!("1. Ban hay cam thuoc day, dung o cac moc: 1m, 2m, 3m, 4m, 5m.");
    println!("2. Mat huong ve camera, co the dung hoac ngoi.");
    println!("3. Ghi lai cac chi so (W, H, Area) hien tren man hinh Console nay.");
    println!("==================================================\n");

    let mut camera_cfg = CameraConfig::default();
    camera_cfg.flip_horizontal = true; 

    // Dung model nhe nhat cho le
    let engine_cfg = EngineConfig::new(
        "models/Body-Detection-Model_640x384.onnx", 
        (640, 384), 
        camera_cfg
    );

    let mut window = Window::new(
        "Calibration Tool - Press ESC to exit",
        640, 480,
        WindowOptions::default(),
    ).expect("Khong the tao cua so");

    window.limit_update_rate(Some(Duration::from_micros(16600)));

    let handle = DetectorPipeline::start(engine_cfg)?;
    let mut display_buffer: Vec<u32> = vec![0; 640 * 480];

    let mut last_log_time = Instant::now();

    while window.is_open() && !window.is_key_down(Key::Escape) {
        if let Some(res) = handle.try_recv() {
            let frame = &res.frame;
            
            // Render hinh anh
            for i in 0..(frame.width * frame.height) as usize {
                let r = frame.rgb[i * 3] as u32;
                let g = frame.rgb[i * 3 + 1] as u32;
                let b = frame.rgb[i * 3 + 2] as u32;
                display_buffer[i] = (r << 16) | (g << 8) | b;
            }

            // Tim nguoi to nhat (gan nhat) de do dac
            let mut largest_box: Option<&BoundingBox> = None;
            let mut max_area = 0.0;

            for bbox in &res.detection.boxes {
                let area = bbox.width * bbox.height;
                if area > max_area {
                    max_area = area;
                    largest_box = Some(bbox);
                }
                // Ve hop xanh la cho moi nguoi
                draw_rect(&mut display_buffer, frame.width as usize, frame.height as usize, bbox, 0x00FF00);
            }

            window.update_with_buffer(&display_buffer, frame.width as usize, frame.height as usize).unwrap();

            // Log so lieu moi 2 giay (chac chan chi in so lieu cua nguoi gan nhat de ban de nhin)
            if last_log_time.elapsed() >= Duration::from_secs(2) {
                if let Some(lb) = largest_box {
                    let w = lb.width as i32;
                    let h = lb.height as i32;
                    let area = w * h;
                    let aspect_ratio = lb.width / lb.height;
                    
                    println!(">>> Thong so: W: {:>3}px | H: {:>3}px | D.Tich: {:>6}px^2 | Ty le (W/H): {:.2}", 
                        w, h, area, aspect_ratio);
                } else {
                    println!(">>> Khong thay ai trong khung hinh...");
                }
                last_log_time = Instant::now();
            }

        } else {
            window.update();
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    Ok(())
}

fn draw_rect(buffer: &mut [u32], w: usize, h: usize, bbox: &BoundingBox, color: u32) {
    let x1 = (bbox.x.max(0.0) as usize).min(w - 1);
    let y1 = (bbox.y.max(0.0) as usize).min(h - 1);
    let x2 = ((bbox.x + bbox.width).max(0.0) as usize).min(w - 1);
    let y2 = ((bbox.y + bbox.height).max(0.0) as usize).min(h - 1);

    for x in x1..=x2 {
        buffer[y1 * w + x] = color;
        buffer[y2 * w + x] = color;
    }
    for y in y1..=y2 {
        buffer[y * w + x1] = color;
        buffer[y * w + x2] = color;
    }
}
