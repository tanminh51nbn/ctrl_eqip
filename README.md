# ctrl_eqip: Edge AI Device Control Framework

`ctrl_eqip` is a high-performance Rust library designed for real-time human detection and automated equipment control on edge devices. It combines ONNX-powered computer vision with geometry-based spatial reasoning and robust industrial communication protocols.

---

## 🚀 Key Features

- **LIFO Processing Pipeline**: Specialized multi-threaded architecture that prioritizes the latest frame (Last-In-First-Out), ensuring zero-latency responsiveness even when AI processing is slower than the camera capture rate.
- **YOLOv8/v11 Optimized**: Seamless integration with modern YOLO models via ONNX Runtime (`ort`), supporting Letterbox preprocessing and automatic coordinate transformation.
- **Spatial Intelligence**: Built-in distance estimation using pinhole camera geometry, allowing for precise proximity-based logic (e.g., "activate fan if person is within 2 meters").
- **Reliable Communication**: Industrial-grade framed binary protocol with CRC8 checksums for noise-resistant communication with microcontrollers like ESP32-C3.
- **Hardware Accelerated**: Native support for CUDA, TensorRT, CoreML, and OpenVINO via the ONNX Runtime provider system.

---

## 🏗️ Architecture

The framework is organized into four logical layers:

- **`engine`**: Handles hardware interfaces (Camera) and AI inference (Detector).
- **`logic`**: Fuses raw detections into actionable insights (Presence tracking, Distance estimation).
- **`protocol`**: Manages binary message framing, encoding, and CRC validation.
- **`transport`**: Handles physical data delivery (Serial/UART) with pluggable backends.

---

## 🛠️ Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- An ONNX model (e.g., `yolov8n.onnx`)
- A webcam or RTSP stream

### Quick Start Example

Add to your `Cargo.toml`:
```toml
[dependencies]
ctrl_eqip = { git = "..." }
```

Run the built-in visualization example:
```bash
# Ensure your model is at 'models/Body-Detection-Model_640x384.onnx'
cargo run --release --example main_loop
```

### Basic Usage

```rust
use ctrl_eqip::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Configure the AI Pipeline
    let config = EngineConfig::new(
        "models/my_model.onnx", 
        (640, 384), 
        CameraConfig::default()
    );

    // 2. Start the processing threads
    let mut handle = DetectorPipeline::start(config)?;

    // 3. Receive real-time results
    while let Some(res) = handle.recv_timeout(Duration::from_millis(100)) {
        if res.detection.has_person {
            println!("Person detected at {} meters", res.detection.camera_fps);
        }
    }
    
    Ok(())
}
```

---

## 🧪 Testing

The library includes a comprehensive suite of unit and integration tests:

```bash
cargo test
```

---

## 📄 License

This project is licensed under the MIT License - see the LICENSE file for details.
