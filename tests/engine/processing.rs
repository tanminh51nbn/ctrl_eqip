use ctrl_eqip::engine::processing::{postprocess, LetterboxMeta};
use ndarray::Array3;

#[test]
fn test_postprocess_empty() {
    let output = Array3::<f32>::zeros((1, 6, 100)); // 100 anchors, 6 features
    let meta = LetterboxMeta { scale: 1.0, pad_w: 0, pad_h: 0 };
    let boxes = postprocess(&output.view(), &meta, 0.5, 0.45);
    assert!(boxes.is_empty());
}
