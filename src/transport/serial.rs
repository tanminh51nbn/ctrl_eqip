//! # SerialTransport
//!
//! Implements the [`Transport`] trait over a USB/UART serial connection to the ESP32-C3.
//!
//! ## Configuration
//! - **Baud rate**: 115200
//! - **Format**: 8N1 (8 data bits, No parity, 1 stop bit)
//! - **Read timeout**: 10 ms (non-blocking poll)
//!
//! ## Usage
//! ```no_run
//! use ctrl_eqip::transport::{Transport, SerialTransport};
//! use ctrl_eqip::protocol::messages::Message;
//!
//! // List available ports:
//! let ports = SerialTransport::list_ports().unwrap();
//! println!("Available ports: {:?}", ports);
//!
//! // Open the ESP32 port (e.g. "COM3" on Windows, "/dev/ttyUSB0" on Linux):
//! let mut transport = SerialTransport::open("COM3").unwrap();
//!
//! // Send a command:
//! transport.send(&Message::FanCommand { speed: 200 }).unwrap();
//!
//! // Non-blocking receive loop:
//! loop {
//!     if let Some(msg) = transport.receive().unwrap() {
//!         println!("Got: {:?}", msg);
//!     }
//! }
//! ```

use std::io::Read;
use std::time::Duration;

use serialport::SerialPort;
use thiserror::Error;

use crate::protocol::{
    codec::{encode, Codec, CodecError, DecodeResult},
    messages::Message,
};
use super::Transport;

/// Baud rate used on both the Laptop and ESP32-C3 sides.
pub const BAUD_RATE: u32 = 115_200;

/// Read timeout for each poll call (non-blocking feel at 10 ms).
const READ_TIMEOUT_MS: u64 = 10;

/// Temporary read buffer size (bytes per OS read call).
const READ_BUF_SIZE: usize = 256;

/// Errors specific to the serial transport.
#[derive(Debug, Error)]
pub enum SerialError {
    #[error("failed to open serial port '{port}': {source}")]
    OpenFailed {
        port: String,
        #[source]
        source: serialport::Error,
    },

    #[error("serial I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to list serial ports: {0}")]
    ListFailed(#[from] serialport::Error),

    #[error("codec error: {0}")]
    Codec(#[from] CodecError),

    #[error("frame encode error: {0}")]
    Encode(String),
}

/// Serial transport — connects to the ESP32-C3 over USB/UART.
pub struct SerialTransport {
    port: Box<dyn SerialPort>,
    codec: Codec,
}

impl SerialTransport {
    /// Open a serial port by name (e.g. `"COM3"` on Windows, `"/dev/ttyUSB0"` on Linux).
    ///
    /// Configures 115200 baud, 8N1, 10 ms read timeout.
    pub fn open(port_name: &str) -> Result<Self, SerialError> {
        let port = serialport::new(port_name, BAUD_RATE)
            .timeout(Duration::from_millis(READ_TIMEOUT_MS))
            .data_bits(serialport::DataBits::Eight)
            .stop_bits(serialport::StopBits::One)
            .parity(serialport::Parity::None)
            .open()
            .map_err(|source| SerialError::OpenFailed {
                port: port_name.to_string(),
                source,
            })?;

        log::info!("[SerialTransport] Opened port '{}' at {} baud", port_name, BAUD_RATE);

        Ok(Self {
            port,
            codec: Codec::new(),
        })
    }

    /// Returns a list of available serial port names on this machine.
    ///
    /// Useful for auto-detecting which port the ESP32 is connected to.
    pub fn list_ports() -> Result<Vec<String>, SerialError> {
        let ports = serialport::available_ports()?;
        let names: Vec<String> = ports.into_iter().map(|p| p.port_name).collect();
        Ok(names)
    }

    /// Auto-open the first available port whose description contains typical
    /// ESP32/CP210x/CH340 identifiers.
    ///
    /// Falls back to the first available port if no known identifier is found.
    /// Returns `None` if no ports are available at all.
    pub fn auto_detect() -> Result<Option<Self>, SerialError> {
        let ports = serialport::available_ports()?;

        // Known USB-serial chip descriptions used by ESP32 dev boards
        let esp32_hints = ["CP210", "CH340", "CH341", "FTDI", "USB Serial", "USB-Serial"];

        // Try to find a port matching known ESP32 descriptions first
        let chosen = ports.iter().find(|p| {
            if let serialport::SerialPortType::UsbPort(info) = &p.port_type {
                let desc = info.product.as_deref().unwrap_or("");
                let mfr = info.manufacturer.as_deref().unwrap_or("");
                esp32_hints.iter().any(|hint| desc.contains(hint) || mfr.contains(hint))
            } else {
                false
            }
        }).or_else(|| ports.first());

        match chosen {
            Some(port_info) => {
                log::info!("[SerialTransport] Auto-detected port: {}", port_info.port_name);
                Ok(Some(Self::open(&port_info.port_name)?))
            }
            None => {
                log::warn!("[SerialTransport] No serial ports found");
                Ok(None)
            }
        }
    }

    /// Flush the internal codec buffer (call after reconnect).
    pub fn reset_codec(&mut self) {
        self.codec.reset();
    }
}

impl Transport for SerialTransport {
    type Error = SerialError;

    /// Encodes the message and writes the full frame to the serial port.
    fn send(&mut self, msg: &Message) -> Result<(), SerialError> {
        let frame = encode(msg).map_err(|e| SerialError::Encode(e.to_string()))?;
        use std::io::Write;
        self.port.write_all(&frame)?;
        log::debug!("[SerialTransport] Sent {:?} ({} bytes)", msg, frame.len());
        Ok(())
    }

    /// Polls the serial port for new bytes, feeds them into the codec, and
    /// returns the first complete decoded message (if any).
    ///
    /// This is **non-blocking** in the sense that it returns `None` immediately
    /// when no complete frame is available. The read timeout (10 ms) is set on
    /// the port so each call waits at most that long.
    fn receive(&mut self) -> Result<Option<Message>, SerialError> {
        // Drain available bytes into the codec buffer
        let mut buf = [0u8; READ_BUF_SIZE];
        match self.port.read(&mut buf) {
            Ok(n) if n > 0 => {
                self.codec.feed(&buf[..n]);
                log::trace!("[SerialTransport] Read {} bytes", n);
            }
            Ok(_) => {}
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => return Err(SerialError::Io(e)),
        }

        // Try to decode one message
        loop {
            match self.codec.decode_one() {
                Some(DecodeResult::Complete { message, .. }) => {
                    log::debug!("[SerialTransport] Received {:?}", message);
                    return Ok(Some(message));
                }
                Some(DecodeResult::Incomplete) | None => return Ok(None),
                Some(DecodeResult::Error { error, .. }) => {
                    // Log and continue — codec already skipped past the bad frame
                    log::warn!("[SerialTransport] Codec error: {}", error);
                    // Continue the loop in case there are more frames in the buffer
                }
            }
        }
    }
}
