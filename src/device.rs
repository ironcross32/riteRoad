//! HID transport for the RØDE Wireless Micro receiver.

use std::fmt;
use std::time::{Duration, Instant};

use hidapi::{HidApi, HidDevice};

use crate::protocol::{self, *};

const REPLY_TIMEOUT: Duration = Duration::from_millis(500);
const INPUT_REPORT_LEN: usize = 64;
/// Vendor-defined usage pages start here.
const VENDOR_USAGE_PAGE: u16 = 0xff00;

#[derive(Debug)]
pub enum Error {
	NotFound,
	Hid(hidapi::HidError),
	Timeout,
	Protocol(ProtocolError),
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::NotFound => write!(f, "Wireless Micro receiver not found"),
			Self::Hid(e) => write!(f, "USB communication error: {e}"),
			Self::Timeout => write!(f, "receiver did not reply"),
			Self::Protocol(e) => e.fmt(f),
		}
	}
}

impl std::error::Error for Error {}

impl From<hidapi::HidError> for Error {
	fn from(e: hidapi::HidError) -> Self {
		Self::Hid(e)
	}
}

impl From<ProtocolError> for Error {
	fn from(e: ProtocolError) -> Self {
		Self::Protocol(e)
	}
}

pub type Result<T> = std::result::Result<T, Error>;

/// An open connection to the receiver.
pub struct Receiver {
	dev: HidDevice,
}

impl Receiver {
	/// Find and open the receiver's control interface.
	pub fn open(api: &HidApi) -> Result<Self> {
		let info = api
			.device_list()
			.filter(|d| d.vendor_id() == VENDOR_ID && d.product_id() == PRODUCT_ID)
			// Prefer the vendor-defined collection if several are listed.
			.max_by_key(|d| d.usage_page() >= VENDOR_USAGE_PAGE)
			.ok_or(Error::NotFound)?;
		let dev = info.open_device(api)?;
		Ok(Self { dev })
	}

	/// Discard any input reports already queued for this handle.
	///
	/// Windows delivers every input report to every open handle, so replies
	/// to other programs (e.g. RØDE Central) pile up here between requests.
	fn drain(&self) -> Result<()> {
		let mut buf = [0u8; INPUT_REPORT_LEN];
		while self.dev.read_timeout(&mut buf, 0)? > 0 {}
		Ok(())
	}

	/// Send a request and wait for its reply, skipping unrelated reports.
	fn transact(&self, req: &Request) -> Result<Vec<u8>> {
		self.drain()?;
		self.dev.write(&req.bytes)?;
		let deadline = Instant::now() + REPLY_TIMEOUT;
		let mut buf = [0u8; INPUT_REPORT_LEN];
		loop {
			let left = deadline.saturating_duration_since(Instant::now());
			if left.is_zero() {
				return Err(Error::Timeout);
			}
			let n = self.dev.read_timeout(&mut buf, left.as_millis() as i32)?;
			if n > 0 && req.matches(&buf[..n]) {
				return Ok(buf[..n].to_vec());
			}
		}
	}

	/// Read a setting and decode it. Retries once if the value is not one we
	/// know: another program's write acknowledgement (`<id> <cmd> 41`, no
	/// value) can arrive while we wait and look like a read reply of 0x00.
	fn read<T>(&self, req: Request, decode: fn(u8) -> std::result::Result<T, ProtocolError>) -> Result<T> {
		let mut last_err = None;
		for _ in 0..2 {
			match decode(protocol::read_value(&self.transact(&req)?)?) {
				Ok(v) => return Ok(v),
				Err(e) => last_err = Some(e),
			}
		}
		Err(last_err.expect("loop runs at least once").into())
	}

	fn write(&self, req: Request) -> Result<()> {
		protocol::check_ack(&self.transact(&req)?)?;
		Ok(())
	}

	pub fn output_mode(&self) -> Result<OutputMode> {
		self.read(read_output_mode(), OutputMode::from_byte)
	}

	pub fn set_output_mode(&self, mode: OutputMode) -> Result<()> {
		self.write(write_output_mode(mode))
	}

	pub fn output_gain(&self) -> Result<OutputGain> {
		self.read(read_output_gain(), OutputGain::from_byte)
	}

	pub fn set_output_gain(&self, gain: OutputGain) -> Result<()> {
		self.write(write_output_gain(gain))
	}

	pub fn led(&self) -> Result<LedBrightness> {
		self.read(read_led(), LedBrightness::from_byte)
	}

	pub fn set_led(&self, led: LedBrightness) -> Result<()> {
		self.write(write_led(led))
	}

	pub fn tx_status(&self, tx: Transmitter) -> Result<TxStatus> {
		Ok(parse_tx_status(&self.transact(&read_tx_status(tx))?)?)
	}

	pub fn set_noise_reduction(&self, tx: Transmitter, nr: NoiseReduction) -> Result<()> {
		self.write(write_noise_reduction(tx, nr))
	}
}
