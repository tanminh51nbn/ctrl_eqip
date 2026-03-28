//! # Protocol Module
//!
//! Defines the binary communication protocol between the Laptop (AI Node) and
//! the ESP32-C3 (Edge/Control Node).
//!
//! ## Quick start
//! ```rust
//! use ctrl_eqip::protocol::{codec::{encode, Codec, DecodeResult}, messages::Message};
//!
//! // Encode a message to bytes:
//! let frame = encode(&Message::FanCommand { speed: 180 }).unwrap();
//!
//! // Parse incoming bytes:
//! let mut codec = Codec::new();
//! codec.feed(&frame);
//! if let Some(DecodeResult::Complete { message, .. }) = codec.decode_one() {
//!     println!("Received: {:?}", message);
//! }
//! ```

pub mod codec;
pub mod messages;

// Re-export common types for convenient use from parent crate
pub use codec::{encode, Codec, DecodeResult};
pub use messages::{ErrorCode, Message, MessageError, FRAME_END, FRAME_START};
