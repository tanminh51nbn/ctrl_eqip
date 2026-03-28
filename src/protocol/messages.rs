//! # Protocol Messages
//!
//! Defines all message types exchanged between the Laptop (AI Node) and ESP32-C3 (Edge Node).
//!
//! ## Frame Format
//! ```text
//! ┌──────────┬──────────┬─────────┬───────────────┬──────────┬──────────┐
//! │ START    │ MSG_TYPE │ LENGTH  │   PAYLOAD     │  CRC8    │  END     │
//! │ 0xAA     │ 1 byte   │ 1 byte  │  0-250 bytes  │  1 byte  │  0x55    │
//! └──────────┴──────────┴─────────┴───────────────┴──────────┴──────────┘
//! ```
//!
//! ## Message Types
//! | ID   | Direction   | Name          | Payload                          |
//! |------|-------------|---------------|----------------------------------|
//! | 0x01 | ESP→Laptop  | SENSOR_DATA   | temp_raw: u16 (°C × 100)         |
//! | 0x02 | Laptop→ESP  | FAN_COMMAND   | speed: u8 (0–255 PWM duty)       |
//! | 0x03 | Laptop→ESP  | FAN_OFF       | (no payload)                     |
//! | 0x10 | Both        | HEARTBEAT     | uptime_ms: u32                   |
//! | 0x11 | Both        | ACK           | acked_type: u8                   |
//! | 0xFE | Both        | ERROR         | error_code: u8                   |

use thiserror::Error;

/// Frame delimiter constants
pub const FRAME_START: u8 = 0xAA;
pub const FRAME_END: u8 = 0x55;

/// Maximum payload size (bytes)
pub const MAX_PAYLOAD_LEN: usize = 250;

/// Protocol-level error codes (sent in ERROR messages)
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    /// Received a frame with invalid CRC
    CrcMismatch = 0x01,
    /// Received an unknown message type
    UnknownMessageType = 0x02,
    /// Payload length out of expected range
    InvalidPayloadLength = 0x03,
    /// ESP32 sensor read failure
    SensorReadFailure = 0x04,
    /// Generic / unknown error
    Unknown = 0xFF,
}

impl From<u8> for ErrorCode {
    fn from(v: u8) -> Self {
        match v {
            0x01 => Self::CrcMismatch,
            0x02 => Self::UnknownMessageType,
            0x03 => Self::InvalidPayloadLength,
            0x04 => Self::SensorReadFailure,
            _ => Self::Unknown,
        }
    }
}

/// All message variants in the protocol.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// ESP32 → Laptop: temperature sensor reading.
    ///
    /// `temp_raw` = actual_temp_celsius × 100  (e.g. 2573 → 25.73°C)
    /// Using integer × 100 avoids floating-point on the MCU.
    SensorData { temp_raw: u16 },

    /// Laptop → ESP32: set fan PWM duty cycle.
    ///
    /// `speed` = 0 (off) … 255 (full speed)
    FanCommand { speed: u8 },

    /// Laptop → ESP32: turn fan off immediately.
    FanOff,

    /// Either direction: keep-alive / connectivity check.
    ///
    /// `uptime_ms` = sender's uptime in milliseconds (wraps at u32::MAX ~49 days)
    Heartbeat { uptime_ms: u32 },

    /// Either direction: acknowledge a previously received message.
    ///
    /// `acked_type` = the MSG_TYPE byte of the message being acknowledged
    Ack { acked_type: u8 },

    /// Either direction: signal an error condition.
    Error { code: ErrorCode },
}

/// Wire-level type byte for each message variant.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageTypeId {
    SensorData = 0x01,
    FanCommand = 0x02,
    FanOff = 0x03,
    Heartbeat = 0x10,
    Ack = 0x11,
    Error = 0xFE,
}

/// Error returned when parsing or serializing messages.
#[derive(Debug, Error)]
pub enum MessageError {
    #[error("unknown message type byte: 0x{0:02X}")]
    UnknownType(u8),

    #[error("payload too short: expected {expected} bytes, got {got}")]
    PayloadTooShort { expected: usize, got: usize },

    #[error("payload too long: {0} bytes exceeds maximum of {MAX_PAYLOAD_LEN}")]
    PayloadTooLong(usize),
}

impl Message {
    /// Returns the wire-level type byte for this message.
    pub fn type_id(&self) -> u8 {
        match self {
            Message::SensorData { .. } => MessageTypeId::SensorData as u8,
            Message::FanCommand { .. } => MessageTypeId::FanCommand as u8,
            Message::FanOff => MessageTypeId::FanOff as u8,
            Message::Heartbeat { .. } => MessageTypeId::Heartbeat as u8,
            Message::Ack { .. } => MessageTypeId::Ack as u8,
            Message::Error { .. } => MessageTypeId::Error as u8,
        }
    }

    /// Serializes the payload portion of this message into a byte buffer.
    ///
    /// Returns the payload bytes. For messages with no payload, returns an empty Vec.
    pub fn encode_payload(&self) -> Vec<u8> {
        match self {
            Message::SensorData { temp_raw } => temp_raw.to_be_bytes().to_vec(),
            Message::FanCommand { speed } => vec![*speed],
            Message::FanOff => vec![],
            Message::Heartbeat { uptime_ms } => uptime_ms.to_be_bytes().to_vec(),
            Message::Ack { acked_type } => vec![*acked_type],
            Message::Error { code } => vec![*code as u8],
        }
    }

    /// Deserializes a message from its type byte and raw payload bytes.
    ///
    /// Returns `Err(MessageError)` if the type is unknown or payload is malformed.
    pub fn decode(type_byte: u8, payload: &[u8]) -> Result<Self, MessageError> {
        match type_byte {
            0x01 => {
                // SensorData: 2 bytes big-endian u16
                if payload.len() < 2 {
                    return Err(MessageError::PayloadTooShort {
                        expected: 2,
                        got: payload.len(),
                    });
                }
                let temp_raw = u16::from_be_bytes([payload[0], payload[1]]);
                Ok(Message::SensorData { temp_raw })
            }
            0x02 => {
                // FanCommand: 1 byte speed
                if payload.is_empty() {
                    return Err(MessageError::PayloadTooShort {
                        expected: 1,
                        got: 0,
                    });
                }
                Ok(Message::FanCommand { speed: payload[0] })
            }
            0x03 => {
                // FanOff: no payload required
                Ok(Message::FanOff)
            }
            0x10 => {
                // Heartbeat: 4 bytes big-endian u32
                if payload.len() < 4 {
                    return Err(MessageError::PayloadTooShort {
                        expected: 4,
                        got: payload.len(),
                    });
                }
                let uptime_ms = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
                Ok(Message::Heartbeat { uptime_ms })
            }
            0x11 => {
                // Ack: 1 byte (the type being acked)
                if payload.is_empty() {
                    return Err(MessageError::PayloadTooShort {
                        expected: 1,
                        got: 0,
                    });
                }
                Ok(Message::Ack { acked_type: payload[0] })
            }
            0xFE => {
                // Error: 1 byte error code
                if payload.is_empty() {
                    return Err(MessageError::PayloadTooShort {
                        expected: 1,
                        got: 0,
                    });
                }
                Ok(Message::Error { code: ErrorCode::from(payload[0]) })
            }
            unknown => Err(MessageError::UnknownType(unknown)),
        }
    }

    /// Convenience: get temperature in °C as f32 from a SensorData message.
    ///
    /// Returns `None` if this is not a SensorData message.
    pub fn temperature_celsius(&self) -> Option<f32> {
        if let Message::SensorData { temp_raw } = self {
            Some(*temp_raw as f32 / 100.0)
        } else {
            None
        }
    }
}

