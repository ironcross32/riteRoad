//! Talks to the receiver on a background thread, so a slow or missing device never blocks the
//! window. Reads everything every [`POLL_INTERVAL`] into a shared [`Snapshot`], and carries out
//! [`Command`]s from the window between readings, one at a time.

use std::{
	sync::{
		Arc, Mutex, PoisonError,
		mpsc::{self, Receiver as Rx, RecvTimeoutError, Sender},
	},
	thread,
	time::Duration,
};

use hidapi::HidApi;
use riteroad::{Error, LedBrightness, NoiseReduction, OutputGain, OutputMode, Receiver, Transmitter, device::Result};

use crate::status::{ReceiverState, Snapshot};

const POLL_INTERVAL: Duration = Duration::from_millis(500);
/// Consecutive failed readings before the receiver counts as gone. One can fail without the
/// receiver going anywhere, such as when another program talks to it at the same moment.
const FAILURES_BEFORE_LOST: u32 = 2;

/// The newest reading, written by the poller and read by the window and the hotkey.
pub type Latest = Arc<Mutex<Snapshot>>;

pub fn read_latest(latest: &Latest) -> Snapshot {
	*latest.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Debug, Clone)]
pub enum Command {
	SetMode(OutputMode),
	SetGain(OutputGain),
	SetLed(LedBrightness),
	/// Written to each transmitter in turn.
	SetNoiseReduction(Vec<Transmitter>, NoiseReduction),
	Stop,
}

pub struct Poller {
	pub latest: Latest,
	pub commands: Sender<Command>,
	/// Why a command failed, as a sentence to speak.
	pub failures: Rx<String>,
}

pub fn spawn() -> Poller {
	let latest: Latest = Arc::new(Mutex::new(Snapshot::Starting));
	let (commands, command_rx) = mpsc::channel();
	let (failure_tx, failures) = mpsc::channel();
	let shared = Arc::clone(&latest);
	thread::spawn(move || run(&shared, &command_rx, &failure_tx));
	Poller { latest, commands, failures }
}

fn run(latest: &Latest, commands: &Rx<Command>, failures: &Sender<String>) {
	let mut api = HidApi::new().ok();
	let mut receiver: Option<Receiver> = None;
	let mut failed_reads = 0;
	loop {
		if receiver.is_none()
			&& let Some(api) = api.as_mut()
			&& api.refresh_devices().is_ok()
		{
			receiver = Receiver::open(api).ok();
		}
		let snapshot = match receiver.as_ref().map(read_state) {
			Some(Ok(state)) => {
				failed_reads = 0;
				Some(Snapshot::Receiver(state))
			}
			Some(Err(_)) => {
				failed_reads += 1;
				if failed_reads >= FAILURES_BEFORE_LOST {
					receiver = None;
					failed_reads = 0;
					Some(Snapshot::NoReceiver)
				} else {
					// Keep showing the last good reading for now.
					None
				}
			}
			None => Some(Snapshot::NoReceiver),
		};
		if let Some(snapshot) = snapshot {
			*latest.lock().unwrap_or_else(PoisonError::into_inner) = snapshot;
		}

		// Wait for the next reading, doing any commands that arrive meanwhile. A command is followed
		// by a fresh reading straight away, so the window catches up quickly.
		match commands.recv_timeout(POLL_INTERVAL) {
			Ok(Command::Stop) | Err(RecvTimeoutError::Disconnected) => return,
			Ok(command) => {
				let mut next = Some(command);
				while let Some(command) = next {
					if matches!(command, Command::Stop) {
						return;
					}
					if let Err(message) = execute(receiver.as_ref(), &command) {
						let _ = failures.send(message);
					}
					next = commands.try_recv().ok();
				}
			}
			Err(RecvTimeoutError::Timeout) => {}
		}
	}
}

fn read_state(rx: &Receiver) -> Result<ReceiverState> {
	Ok(ReceiverState {
		mode: rx.output_mode()?,
		gain: rx.output_gain()?,
		led: rx.led()?,
		tx1: rx.tx_status(Transmitter::Tx1)?,
		tx2: rx.tx_status(Transmitter::Tx2)?,
	})
}

fn execute(receiver: Option<&Receiver>, command: &Command) -> std::result::Result<(), String> {
	let Some(rx) = receiver else {
		return Err("Couldn't change the setting: the receiver is not connected.".into());
	};
	let failed = |what: &str, error: Error| format!("Couldn't change {what}: {error}.");
	match command {
		Command::SetMode(mode) => rx.set_output_mode(*mode).map_err(|e| failed("the output mode", e)),
		Command::SetGain(gain) => rx.set_output_gain(*gain).map_err(|e| failed("the output gain", e)),
		Command::SetLed(led) => rx.set_led(*led).map_err(|e| failed("the LED brightness", e)),
		Command::SetNoiseReduction(targets, level) => {
			for &tx in targets {
				rx.set_noise_reduction(tx, *level).map_err(|e| failed(&format!("noise reduction on {tx}"), e))?;
			}
			Ok(())
		}
		Command::Stop => Ok(()),
	}
}
