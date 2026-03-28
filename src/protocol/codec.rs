//! # Codec — Frame Encoder / Decoder
//!
//! Handles framing of [`Message`]s into raw bytes for transmission over serial,
//! and parses incoming byte streams back into [`Message`]s.
//!
//! ## Frame structure
//! ```text
//! [0xAA] [TYPE] [LEN] [PAYLOAD × LEN bytes] [CRC8] [0x55]
//!   1B     1B    1B       0–250 B              1B     1B
//! ```
//!
//! CRC8 is computed over: TYPE + LEN + PAYLOAD (not including START/END bytes).

use crc::{Crc, CRC_8_SMBUS};
use thiserror::Error;

use super::messages::{Message, MessageError, FRAME_END, FRAME_START, MAX_PAYLOAD_LEN};

// ─── CRC ─────────────────────────────────────────────────────────────────────

/// CRC-8/SMBUS algorithm — widely supported, easy to implement on ESP32 in C.
const CRC8: Crc<u8> = Crc::<u8>::new(&CRC_8_SMBUS);

/// Compute CRC8 over a byte slice.
pub fn crc8(data: &[u8]) -> u8 {
    CRC8.checksum(data)
}

// ─── Encode ──────────────────────────────────────────────────────────────────

/// Errors that can occur during codec operations.
#[derive(Debug, Error)]
pub enum CodecError {
    #[error("payload too long: {0} > {MAX_PAYLOAD_LEN}")]
    PayloadTooLong(usize),

    #[error("message parse error: {0}")]
    MessageError(#[from] MessageError),

    #[error("frame incomplete — need more bytes")]
    Incomplete,

    #[error("frame alignment lost — no START byte found")]
    NoStartByte,

    #[error("CRC mismatch: expected 0x{expected:02X}, got 0x{got:02X}")]
    CrcMismatch { expected: u8, got: u8 },

    #[error("frame missing END byte (0x55)")]
    MissingEndByte,
}

/// Encode a [`Message`] into a complete wire frame (Vec<u8>).
///
/// # Errors
/// Returns [`CodecError::PayloadTooLong`] if the payload exceeds [`MAX_PAYLOAD_LEN`].
///
/// # Example
/// ```
/// use ctrl_eqip::protocol::{messages::Message, codec::encode};
///
/// let frame = encode(&Message::FanCommand { speed: 128 }).unwrap();
/// ```
pub fn encode(msg: &Message) -> Result<Vec<u8>, CodecError> {
    let payload = msg.encode_payload();
    let type_byte = msg.type_id();

    if payload.len() > MAX_PAYLOAD_LEN {
        return Err(CodecError::PayloadTooLong(payload.len()));
    }

    let len_byte = payload.len() as u8;

    // CRC is computed over: TYPE | LEN | PAYLOAD
    let mut crc_input = Vec::with_capacity(2 + payload.len());
    crc_input.push(type_byte);
    crc_input.push(len_byte);
    crc_input.extend_from_slice(&payload);
    let checksum = crc8(&crc_input);

    // Frame: START | TYPE | LEN | PAYLOAD | CRC | END
    let mut frame = Vec::with_capacity(6 + payload.len());
    frame.push(FRAME_START);
    frame.push(type_byte);
    frame.push(len_byte);
    frame.extend_from_slice(&payload);
    frame.push(checksum);
    frame.push(FRAME_END);

    Ok(frame)
}

// ─── Decode ──────────────────────────────────────────────────────────────────

/// Result of attempting to decode from a buffer.
#[derive(Debug)]
pub enum DecodeResult {
    /// A complete frame was successfully decoded; also returns how many bytes were consumed.
    Complete { message: Message, bytes_consumed: usize },
    /// Not enough bytes yet; caller should append more data and retry.
    Incomplete,
    /// A framing error occurred (e.g. CRC fail). Returns how many bytes to skip.
    Error { error: CodecError, skip: usize },
}

/// Codec with an internal receive buffer for accumulating partial frames.
///
/// # Usage
/// ```
/// use ctrl_eqip::protocol::codec::Codec;
///
/// let mut codec = Codec::new();
/// // Feed bytes as they arrive (from serial read):
/// // codec.feed(&incoming_bytes);
/// // while let Some(result) = codec.decode_one() { ... }
/// ```
#[derive(Debug, Default)]
pub struct Codec {
    buf: Vec<u8>,
}

impl Codec {
    /// Creates a new empty codec buffer.
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Feed newly received bytes into the codec's internal buffer.
    pub fn feed(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }

    /// Attempt to decode one complete message from the buffer.
    ///
    /// Returns `None` when there are no complete frames available.
    /// The caller should call this in a loop until `None` is returned.
    ///
    /// On error, the codec discards bytes up to and including the bad frame
    /// start, so misaligned data is automatically recovered.
    pub fn decode_one(&mut self) -> Option<DecodeResult> {
        if self.buf.is_empty() {
            return None;
        }

        // Find the START byte
        let start_pos = match self.buf.iter().position(|&b| b == FRAME_START) {
            Some(p) => p,
            None => {
                // No start byte => discard everything
                self.buf.clear();
                return Some(DecodeResult::Error {
                    error: CodecError::NoStartByte,
                    skip: 0,
                });
            }
        };

        // Discard bytes before the START byte
        if start_pos > 0 {
            self.buf.drain(..start_pos);
        }

        // Minimum frame: START(1) + TYPE(1) + LEN(1) + CRC(1) + END(1) = 5 bytes
        if self.buf.len() < 5 {
            return Some(DecodeResult::Incomplete);
        }

        // Parse the LEN byte (index 2) to know total frame size
        let len = self.buf[2] as usize;
        let frame_total = 1 + 1 + 1 + len + 1 + 1; // START TYPE LEN PAYLOAD CRC END

        if self.buf.len() < frame_total {
            return Some(DecodeResult::Incomplete);
        }

        // We have a complete frame in the buffer
        let type_byte = self.buf[1];
        let payload = self.buf[3..3 + len].to_vec();
        let received_crc = self.buf[3 + len];
        let end_byte = self.buf[4 + len];

        // Validate END byte
        if end_byte != FRAME_END {
            // Skip past this spurious start byte
            self.buf.drain(..1);
            return Some(DecodeResult::Error {
                error: CodecError::MissingEndByte,
                skip: 1,
            });
        }

        // Validate CRC
        let mut crc_input = Vec::with_capacity(2 + len);
        crc_input.push(type_byte);
        crc_input.push(len as u8);
        crc_input.extend_from_slice(&payload);
        let expected_crc = crc8(&crc_input);

        if received_crc != expected_crc {
            self.buf.drain(..frame_total);
            return Some(DecodeResult::Error {
                error: CodecError::CrcMismatch {
                    expected: expected_crc,
                    got: received_crc,
                },
                skip: frame_total,
            });
        }

        // Decode the message
        let bytes_consumed = frame_total;
        self.buf.drain(..frame_total);

        match Message::decode(type_byte, &payload) {
            Ok(message) => Some(DecodeResult::Complete { message, bytes_consumed }),
            Err(e) => Some(DecodeResult::Error {
                error: CodecError::MessageError(e),
                skip: bytes_consumed,
            }),
        }
    }

    /// Returns the number of bytes currently in the internal buffer.
    pub fn buffered_len(&self) -> usize {
        self.buf.len()
    }

    /// Clears the internal buffer (e.g. on reconnect).
    pub fn reset(&mut self) {
        self.buf.clear();
    }
}

