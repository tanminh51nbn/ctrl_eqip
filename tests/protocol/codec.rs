use ctrl_eqip::protocol::codec::{Codec, CodecError, DecodeResult, encode};
use ctrl_eqip::protocol::messages::Message;

fn encode_decode(msg: Message) -> Message {
    let frame = encode(&msg).expect("encode failed");
    let mut codec = Codec::new();
    codec.feed(&frame);
    match codec.decode_one().expect("should have a result") {
        DecodeResult::Complete { message, .. } => message,
        other => panic!("expected Complete, got {:?}", other),
    }
}

#[test]
fn sensor_data_encode_decode() {
    let msg = Message::SensorData { temp_raw: 2573 };
    assert_eq!(encode_decode(msg.clone()), msg);
}

#[test]
fn fan_command_encode_decode() {
    let msg = Message::FanCommand { speed: 200 };
    assert_eq!(encode_decode(msg.clone()), msg);
}

#[test]
fn fan_off_encode_decode() {
    let msg = Message::FanOff;
    assert_eq!(encode_decode(msg.clone()), msg);
}

#[test]
fn heartbeat_encode_decode() {
    let msg = Message::Heartbeat { uptime_ms: 60_000 };
    assert_eq!(encode_decode(msg.clone()), msg);
}

#[test]
fn ack_encode_decode() {
    let msg = Message::Ack { acked_type: 0x01 };
    assert_eq!(encode_decode(msg.clone()), msg);
}

#[test]
fn crc_corruption_detected() {
    let msg = Message::SensorData { temp_raw: 1234 };
    let mut frame = encode(&msg).unwrap();

    // Corrupt the CRC byte (second-to-last byte)
    let crc_pos = frame.len() - 2;
    frame[crc_pos] ^= 0xFF;

    let mut codec = Codec::new();
    codec.feed(&frame);
    match codec.decode_one().unwrap() {
        DecodeResult::Error { error: CodecError::CrcMismatch { .. }, .. } => {} // expected
        other => panic!("expected CRC error, got {:?}", other),
    }
}

#[test]
fn partial_frame_returns_incomplete() {
    let msg = Message::Heartbeat { uptime_ms: 99 };
    let frame = encode(&msg).unwrap();

    // Feed only half the frame
    let mut codec = Codec::new();
    codec.feed(&frame[..frame.len() / 2]);
    assert!(matches!(codec.decode_one(), Some(DecodeResult::Incomplete)));

    // Feed the rest
    codec.feed(&frame[frame.len() / 2..]);
    assert!(matches!(codec.decode_one(), Some(DecodeResult::Complete { .. })));
}

#[test]
fn back_to_back_frames() {
    let msg1 = Message::FanCommand { speed: 100 };
    let msg2 = Message::FanOff;
    let mut combined = encode(&msg1).unwrap();
    combined.extend(encode(&msg2).unwrap());

    let mut codec = Codec::new();
    codec.feed(&combined);

    let r1 = codec.decode_one().unwrap();
    assert!(matches!(r1, DecodeResult::Complete { message: Message::FanCommand { speed: 100 }, .. }));

    let r2 = codec.decode_one().unwrap();
    assert!(matches!(r2, DecodeResult::Complete { message: Message::FanOff, .. }));

    assert!(codec.decode_one().is_none());
}

#[test]
fn garbage_before_frame_skipped() {
    let msg = Message::FanOff;
    let frame = encode(&msg).unwrap();

    let mut data = vec![0x00, 0xFF, 0x13]; // garbage prefix
    data.extend_from_slice(&frame);

    let mut codec = Codec::new();
    codec.feed(&data);
    match codec.decode_one().unwrap() {
        DecodeResult::Complete { message, .. } => assert_eq!(message, Message::FanOff),
        other => panic!("expected Complete, got {:?}", other),
    }
}
