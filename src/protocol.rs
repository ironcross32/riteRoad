//! Message encoding and decoding for the RØDE Wireless Micro receiver.
//!
//! Pure byte handling with no I/O. See `docs/PROTOCOL.md` for how these were
//! worked out.

use std::fmt;

pub const VENDOR_ID: u16 = 0x19f7;
pub const PRODUCT_ID: u16 = 0x006a;

/// Status byte meaning "acknowledged" ('A').
const ACK: u8 = 0x41;

const RECEIVER_REPORT: u8 = 0x09;
const TX_STATUS_REPORT: u8 = 0x01;

const CMD_OUTPUT_MODE: u8 = 0x14;
const CMD_LED: u8 = 0x3d;
const CMD_OUTPUT_GAIN: u8 = 0x3e;
const CMD_NOISE_REDUCTION: u8 = 0x51;
const CMD_TX_STATUS: u8 = 0x55;

const READ: u8 = 0x01;
const WRITE: u8 = 0x00;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
	/// Reply was shorter than the fields we need.
	TooShort,
	/// Device answered with a status other than ACK.
	Nak(u8),
	/// A value byte we don't have a name for.
	UnknownValue { what: &'static str, value: u8 },
}

impl fmt::Display for ProtocolError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::TooShort => write!(f, "reply from the receiver was too short"),
			Self::Nak(s) => write!(f, "receiver rejected the command (status 0x{s:02x})"),
			Self::UnknownValue { what, value } => {
				write!(f, "receiver reported an unknown {what} value 0x{value:02x}")
			}
		}
	}
}

impl std::error::Error for ProtocolError {}

/// Declares a setting enum backed by a wire byte, with display names.
macro_rules! wire_enum {
    ($name:ident, $what:literal, { $($variant:ident = $byte:literal => $label:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name { $($variant),+ }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            pub fn to_byte(self) -> u8 {
                match self { $($name::$variant => $byte),+ }
            }

            pub fn from_byte(b: u8) -> Result<Self, ProtocolError> {
                match b {
                    $($byte => Ok($name::$variant),)+
                    value => Err(ProtocolError::UnknownValue { what: $what, value }),
                }
            }

            pub fn label(self) -> &'static str {
                match self { $($name::$variant => $label),+ }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.label())
            }
        }
    };
}

wire_enum!(OutputMode, "output mode", {
	Split = 0x00 => "Split",
	Merged = 0x01 => "Merged",
});

wire_enum!(OutputGain, "output gain", {
	Low = 0x0a => "Low",
	Medium = 0x0e => "Medium",
	High = 0x10 => "High",
});

wire_enum!(NoiseReduction, "noise reduction", {
	Off = 0x00 => "Off",
	Low = 0x03 => "Low",
	High = 0x06 => "High",
});

wire_enum!(LedBrightness, "LED brightness", {
	Dim = 0x00 => "Dim",
	Bright = 0x01 => "Bright",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Transmitter {
	Tx1,
	Tx2,
}

impl Transmitter {
	pub const ALL: [Transmitter; 2] = [Transmitter::Tx1, Transmitter::Tx2];

	fn to_byte(self) -> u8 {
		match self {
			Self::Tx1 => 0x01,
			Self::Tx2 => 0x02,
		}
	}
}

impl fmt::Display for Transmitter {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(match self {
			Self::Tx1 => "Transmitter 1",
			Self::Tx2 => "Transmitter 2",
		})
	}
}

/// What the receiver reports about one transmitter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxStatus {
	Disconnected,
	Connected {
		/// Battery charge, 0-100.
		battery: u8,
		noise_reduction: NoiseReduction,
		/// Status byte 9; meaning not yet known (possibly link quality).
		unknown_b9: u8,
	},
}

/// One request: the bytes to send and how to recognise its reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
	pub bytes: Vec<u8>,
}

impl Request {
	fn new(bytes: Vec<u8>) -> Self {
		Self { bytes }
	}

	/// Report ID the reply arrives on.
	pub fn reply_id(&self) -> u8 {
		self.bytes[0] + 1
	}

	/// Command byte the reply echoes.
	pub fn command(&self) -> u8 {
		self.bytes[1]
	}

	/// True if `reply` answers this request.
	pub fn matches(&self, reply: &[u8]) -> bool {
		reply.len() >= 2 && reply[0] == self.reply_id() && reply[1] == self.command()
	}
}

// ---- Requests ---------------------------------------------------------------

pub fn read_output_mode() -> Request {
	Request::new(vec![RECEIVER_REPORT, CMD_OUTPUT_MODE, READ])
}

pub fn write_output_mode(mode: OutputMode) -> Request {
	Request::new(vec![RECEIVER_REPORT, CMD_OUTPUT_MODE, WRITE, mode.to_byte()])
}

pub fn read_output_gain() -> Request {
	Request::new(vec![RECEIVER_REPORT, CMD_OUTPUT_GAIN, READ])
}

pub fn write_output_gain(gain: OutputGain) -> Request {
	Request::new(vec![RECEIVER_REPORT, CMD_OUTPUT_GAIN, WRITE, gain.to_byte()])
}

pub fn read_led() -> Request {
	Request::new(vec![RECEIVER_REPORT, CMD_LED, READ])
}

pub fn write_led(led: LedBrightness) -> Request {
	// The extra 0x00 matches what RØDE Central sends; its meaning is unknown.
	Request::new(vec![RECEIVER_REPORT, CMD_LED, WRITE, 0x00, led.to_byte()])
}

pub fn write_noise_reduction(tx: Transmitter, nr: NoiseReduction) -> Request {
	Request::new(vec![RECEIVER_REPORT, CMD_NOISE_REDUCTION, tx.to_byte(), nr.to_byte()])
}

pub fn read_tx_status(tx: Transmitter) -> Request {
	Request::new(vec![TX_STATUS_REPORT, CMD_TX_STATUS, tx.to_byte()])
}

// ---- Reply parsing ----------------------------------------------------------

/// Check the ACK byte and return the reply.
pub fn check_ack(reply: &[u8]) -> Result<&[u8], ProtocolError> {
	match reply.get(2) {
		None => Err(ProtocolError::TooShort),
		Some(&ACK) => Ok(reply),
		Some(&other) => Err(ProtocolError::Nak(other)),
	}
}

/// Value byte of a read reply (`<id> <cmd> 41 <value>`).
pub fn read_value(reply: &[u8]) -> Result<u8, ProtocolError> {
	check_ack(reply)?;
	// Hidapi returns the full zero-padded report, but be lenient with short
	// ones: a missing value byte is a zero.
	Ok(reply.get(3).copied().unwrap_or(0))
}

pub fn parse_tx_status(reply: &[u8]) -> Result<TxStatus, ProtocolError> {
	check_ack(reply)?;
	let byte = |i: usize| reply.get(i).copied().unwrap_or(0);
	if byte(3) == 0x00 {
		return Ok(TxStatus::Disconnected);
	}
	if reply.len() < 11 {
		return Err(ProtocolError::TooShort);
	}
	Ok(TxStatus::Connected {
		battery: byte(10),
		noise_reduction: NoiseReduction::from_byte(byte(12))?,
		unknown_b9: byte(9),
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Pad a captured (zero-trimmed) reply to the 64-byte report size.
	fn report(bytes: &[u8]) -> Vec<u8> {
		let mut v = bytes.to_vec();
		v.resize(64, 0);
		v
	}

	#[test]
	fn requests_match_captured_bytes() {
		assert_eq!(read_output_mode().bytes, [0x09, 0x14, 0x01]);
		assert_eq!(write_output_mode(OutputMode::Merged).bytes, [0x09, 0x14, 0x00, 0x01]);
		assert_eq!(write_output_mode(OutputMode::Split).bytes, [0x09, 0x14, 0x00, 0x00]);
		assert_eq!(read_output_gain().bytes, [0x09, 0x3e, 0x01]);
		assert_eq!(write_output_gain(OutputGain::Medium).bytes, [0x09, 0x3e, 0x00, 0x0e]);
		assert_eq!(read_led().bytes, [0x09, 0x3d, 0x01]);
		assert_eq!(write_led(LedBrightness::Bright).bytes, [0x09, 0x3d, 0x00, 0x00, 0x01]);
		assert_eq!(write_noise_reduction(Transmitter::Tx1, NoiseReduction::High).bytes, [0x09, 0x51, 0x01, 0x06]);
		assert_eq!(read_tx_status(Transmitter::Tx2).bytes, [0x01, 0x55, 0x02]);
	}

	#[test]
	fn reply_matching() {
		let req = read_tx_status(Transmitter::Tx1);
		assert!(req.matches(&report(&[0x02, 0x55, 0x41])));
		assert!(!req.matches(&report(&[0x0a, 0x55, 0x41])));
		assert!(!req.matches(&report(&[0x02, 0x14, 0x41])));
	}

	#[test]
	fn reads_values() {
		assert_eq!(read_value(&report(&[0x0a, 0x3e, 0x41, 0x10])), Ok(0x10));
		assert_eq!(read_value(&report(&[0x0a, 0x14, 0x41])), Ok(0x00));
		assert_eq!(read_value(&report(&[0x0a, 0x01, 0x4e])), Err(ProtocolError::Nak(0x4e)));
		assert_eq!(OutputGain::from_byte(0x10), Ok(OutputGain::High));
	}

	#[test]
	fn parses_connected_status() {
		let r = report(&[0x02, 0x55, 0x41, 0x31, 0x02, 0, 0, 0, 0x23, 0x28, 0x5f, 0x00, 0x06]);
		assert_eq!(
			parse_tx_status(&r),
			Ok(TxStatus::Connected { battery: 95, noise_reduction: NoiseReduction::High, unknown_b9: 0x28 })
		);
	}

	#[test]
	fn parses_disconnected_status() {
		let r = report(&[0x02, 0x55, 0x41, 0x00, 0x00, 0x00, 0x00, 0xff]);
		assert_eq!(parse_tx_status(&r), Ok(TxStatus::Disconnected));
	}
}
