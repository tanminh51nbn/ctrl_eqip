use ctrl_eqip::protocol::messages::{Message, ErrorCode, MessageError};

#[test]
fn sensor_data_round_trip() {
    let msg = Message::SensorData { temp_raw: 2573 }; // 25.73°C
    let payload = msg.encode_payload();
    let decoded = Message::decode(msg.type_id(), &payload).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
fn temperature_conversion() {
    let msg = Message::SensorData { temp_raw: 3500 };
    assert_eq!(msg.temperature_celsius(), Some(35.0));
}

#[test]
fn fan_command_round_trip() {
    let msg = Message::FanCommand { speed: 200 };
    let payload = msg.encode_payload();
    let decoded = Message::decode(msg.type_id(), &payload).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
fn fan_off_round_trip() {
    let msg = Message::FanOff;
    let payload = msg.encode_payload();
    let decoded = Message::decode(msg.type_id(), &payload).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
fn heartbeat_round_trip() {
    let msg = Message::Heartbeat { uptime_ms: 123_456_789 };
    let payload = msg.encode_payload();
    let decoded = Message::decode(msg.type_id(), &payload).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
fn ack_round_trip() {
    let msg = Message::Ack { acked_type: 0x01 };
    let payload = msg.encode_payload();
    let decoded = Message::decode(msg.type_id(), &payload).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
fn error_round_trip() {
    let msg = Message::Error { code: ErrorCode::CrcMismatch };
    let payload = msg.encode_payload();
    let decoded = Message::decode(msg.type_id(), &payload).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
fn unknown_type_returns_error() {
    let result = Message::decode(0x99, &[]);
    assert!(matches!(result, Err(MessageError::UnknownType(0x99))));
}

#[test]
fn short_payload_returns_error() {
    // SensorData needs 2 bytes
    let result = Message::decode(0x01, &[0x10]);
    assert!(matches!(result, Err(MessageError::PayloadTooShort { .. })));
}
