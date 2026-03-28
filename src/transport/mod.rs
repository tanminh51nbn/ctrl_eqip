//! # Transport Module
//!
//! Provides the [`Transport`] trait and implementations for sending/receiving
//! [`Message`]s over different physical layers.
//!
//! Currently implemented:
//! - [`SerialTransport`]: USB/UART serial connection to ESP32-C3.
//!
//! Future: TCP/UDP transport for WiFi can implement the same trait.

pub mod serial;

pub use serial::SerialTransport;

use crate::protocol::messages::Message;

/// A transport channel that can send and receive protocol [`Message`]s.
///
/// Implement this trait to add new transport backends (e.g. WiFi/TCP).
pub trait Transport {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Send a message. Blocks until all bytes are written to the underlying
    /// I/O layer (not necessarily received by the remote end).
    fn send(&mut self, msg: &Message) -> Result<(), Self::Error>;

    /// Non-blocking receive: return the next decoded message if available,
    /// or `None` if no complete frame is ready yet.
    fn receive(&mut self) -> Result<Option<Message>, Self::Error>;
}
