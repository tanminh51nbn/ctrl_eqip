//! # WifiTransport
//!
//! Implements the [`Transport`] trait over a UDP socket.
//!
//! ## Usage
//! ```no_run
//! use ctrl_eqip::transport::{Transport, WifiTransport};
//! use ctrl_eqip::protocol::messages::Message;
//!
//! // Bind a local socket on a random port and connect to ESP32 at IP 192.168.1.100, port 5000.
//! let mut transport = WifiTransport::new("192.168.1.100:5000").unwrap();
//!
//! // Send a command:
//! transport.send(&Message::FanCommand { speed: 200 }).unwrap();
//!
//! // Receive (if ESP32 sends something back)
//! loop {
//!     if let Some(msg) = transport.receive().unwrap() {
//!         println!("Got: {:?}", msg);
//!     }
//! }
//! ```

use std::net::UdpSocket;
use thiserror::Error;

use crate::protocol::{
    codec::{encode, Codec, CodecError, DecodeResult},
    messages::Message,
};
use super::Transport;

const READ_BUF_SIZE: usize = 256;

#[derive(Debug, Error)]
pub enum WifiError {
    #[error("failed to bind local socket: {0}")]
    BindFailed(std::io::Error),

    #[error("failed to connect to remote address '{addr}': {source}")]
    ConnectFailed {
        addr: String,
        #[source]
        source: std::io::Error,
    },

    #[error("udp I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("codec error: {0}")]
    Codec(#[from] CodecError),

    #[error("frame encode error: {0}")]
    Encode(String),
}

/// UDP transport — sends and receives protocol messages over WiFi.
pub struct WifiTransport {
    socket: UdpSocket,
    codec: Codec,
}

impl WifiTransport {
    /// Create a new UDP transport connected to the target `esp_addr`.
    /// `esp_addr` should be in the format "IP:PORT", e.g., "192.168.4.1:5000".
    pub fn new(esp_addr: &str) -> Result<Self, WifiError> {
        // Bind to any local IP and an OS-assigned random port
        let socket = UdpSocket::bind("0.0.0.0:0").map_err(WifiError::BindFailed)?;
        
        // Connect the UDP socket to the target address (ESP32)
        // This makes `send` and `recv` simpler as they don't need the address.
        socket.connect(esp_addr).map_err(|e| WifiError::ConnectFailed {
            addr: esp_addr.to_string(),
            source: e,
        })?;

        // Set non-blocking to allow non-blocking `receive()`
        socket.set_nonblocking(true)?;

        log::info!("[WifiTransport] UDP socket bound and connected to {}", esp_addr);

        Ok(Self {
            socket,
            codec: Codec::new(),
        })
    }
}

impl Transport for WifiTransport {
    type Error = WifiError;

    fn send(&mut self, msg: &Message) -> Result<(), WifiError> {
        let frame = encode(msg).map_err(|e| WifiError::Encode(e.to_string()))?;
        self.socket.send(&frame)?;
        log::debug!("[WifiTransport] Sent {:?} ({} bytes)", msg, frame.len());
        Ok(())
    }

    fn receive(&mut self) -> Result<Option<Message>, WifiError> {
        let mut buf = [0u8; READ_BUF_SIZE];
        
        match self.socket.recv(&mut buf) {
            Ok(n) if n > 0 => {
                self.codec.feed(&buf[..n]);
                log::trace!("[WifiTransport] Read {} bytes", n);
            }
            Ok(_) => {}
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // Non-blocking socket returned WouldBlock, no data available yet
            }
            Err(e) => return Err(WifiError::Io(e)),
        }

        loop {
            match self.codec.decode_one() {
                Some(DecodeResult::Complete { message, .. }) => {
                    log::debug!("[WifiTransport] Received {:?}", message);
                    return Ok(Some(message));
                }
                Some(DecodeResult::Incomplete) | None => return Ok(None),
                Some(DecodeResult::Error { error, .. }) => {
                    log::warn!("[WifiTransport] Codec error: {}", error);
                }
            }
        }
    }
}
