//! What the receiver last reported, and the text and announcements made from it. No I/O or UI,
//! so all of it is unit tested.

use riteroad::{LedBrightness, OutputGain, OutputMode, Transmitter, TxStatus};

pub const NO_RECEIVER_TEXT: &str =
	"The RØDE Wireless Micro receiver is not plugged in. Plug it into a USB port to use riteRoad.";

/// Battery levels that trigger a warning, highest first.
const BATTERY_WARNINGS: [u8; 2] = [20, 10];
/// How far above a warning level the battery must climb before that warning can sound again, so a
/// reading that wobbles between 20 and 21 doesn't repeat it.
const BATTERY_HYSTERESIS: u8 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Snapshot {
	/// Before the first reading.
	Starting,
	NoReceiver,
	Receiver(ReceiverState),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReceiverState {
	pub mode: OutputMode,
	pub gain: OutputGain,
	pub led: LedBrightness,
	pub tx1: TxStatus,
	pub tx2: TxStatus,
}

impl ReceiverState {
	pub const fn tx(&self, tx: Transmitter) -> TxStatus {
		match tx {
			Transmitter::Tx1 => self.tx1,
			Transmitter::Tx2 => self.tx2,
		}
	}
}

const fn number(tx: Transmitter) -> u8 {
	match tx {
		Transmitter::Tx1 => 1,
		Transmitter::Tx2 => 2,
	}
}

/// The status box's contents, one item per line.
pub fn status_text(snapshot: &Snapshot) -> String {
	match snapshot {
		Snapshot::Starting => "Looking for the RØDE Wireless Micro receiver…".into(),
		Snapshot::NoReceiver => NO_RECEIVER_TEXT.into(),
		Snapshot::Receiver(state) => {
			let mut lines = vec![
				"Receiver: connected".to_string(),
				format!("Output mode: {}", state.mode),
				format!("Output gain: {}", state.gain),
				format!("LED brightness: {}", state.led),
			];
			for tx in Transmitter::ALL {
				lines.push(match state.tx(tx) {
					TxStatus::Disconnected => format!("{tx}: not connected"),
					TxStatus::Connected { battery, noise_reduction, .. } => {
						format!("{tx}: connected, battery {battery}%, noise reduction {noise_reduction}")
					}
				});
			}
			lines.join("\n")
		}
	}
}

/// The one-sentence summary the hotkey speaks. It leaves out what is as expected: a connected
/// receiver, and transmitters that are connected (whose battery is given instead).
pub fn summary(snapshot: &Snapshot) -> String {
	let Snapshot::Receiver(state) = snapshot else {
		return "Receiver not connected.".into();
	};
	let mut text = format!("{} mode.", state.mode);
	if Transmitter::ALL.iter().all(|&tx| state.tx(tx) == TxStatus::Disconnected) {
		text.push_str(" No transmitters connected.");
		return text;
	}
	for tx in Transmitter::ALL {
		match state.tx(tx) {
			TxStatus::Disconnected => text.push_str(&format!(" Transmitter {} not connected.", number(tx))),
			TxStatus::Connected { battery, .. } => {
				text.push_str(&format!(" Transmitter {}, {battery} percent.", number(tx)));
			}
		}
	}
	text
}

/// Which settings checkbox an announcement belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
	Receiver,
	Transmitter,
	Battery,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Announcement {
	pub category: Category,
	pub text: String,
}

impl Announcement {
	fn new(category: Category, text: impl Into<String>) -> Self {
		Self { category, text: text.into() }
	}
}

/// Turns successive snapshots into announcements, remembering which battery warnings have
/// already been given.
#[derive(Debug, Default)]
pub struct Announcer {
	/// Per transmitter, the lowest warning level already announced.
	warned: [Option<u8>; 2],
}

impl Announcer {
	pub fn changes(&mut self, previous: &Snapshot, current: &Snapshot) -> Vec<Announcement> {
		let mut out = Vec::new();
		match (previous, current) {
			(Snapshot::Receiver(_), Snapshot::NoReceiver) => {
				out.push(Announcement::new(Category::Receiver, "Receiver disconnected."));
			}
			(Snapshot::NoReceiver, Snapshot::Receiver(_)) => {
				out.push(Announcement::new(Category::Receiver, "Receiver connected."));
			}
			(Snapshot::Receiver(before), Snapshot::Receiver(after)) => {
				for tx in Transmitter::ALL {
					let was = before.tx(tx) != TxStatus::Disconnected;
					let is = after.tx(tx) != TxStatus::Disconnected;
					if was != is {
						let what = if is { "connected" } else { "disconnected" };
						out.push(Announcement::new(
							Category::Transmitter,
							format!("Transmitter {} {what}.", number(tx)),
						));
					}
				}
			}
			_ => {}
		}
		if let Snapshot::Receiver(state) = current {
			for tx in Transmitter::ALL {
				if let TxStatus::Connected { battery, .. } = state.tx(tx)
					&& self.battery_crossed(tx, battery)
				{
					out.push(Announcement::new(
						Category::Battery,
						format!("Transmitter {} battery {battery} percent.", number(tx)),
					));
				}
			}
		}
		out
	}

	/// Whether `battery` has just reached a warning level not yet announced for `tx`.
	fn battery_crossed(&mut self, tx: Transmitter, battery: u8) -> bool {
		let warned = &mut self.warned[usize::from(number(tx) - 1)];
		// Recharged well clear of the level last warned about: it may sound again.
		if let Some(level) = *warned
			&& battery >= level + BATTERY_HYSTERESIS
		{
			*warned = BATTERY_WARNINGS.iter().copied().filter(|&l| battery < l + BATTERY_HYSTERESIS).min();
		}
		let Some(level) = BATTERY_WARNINGS.iter().copied().filter(|&l| battery <= l).min() else {
			return false;
		};
		if warned.is_some_and(|w| w <= level) {
			return false;
		}
		*warned = Some(level);
		true
	}
}

#[cfg(test)]
mod tests {
	use riteroad::NoiseReduction;

	use super::*;

	fn on(battery: u8) -> TxStatus {
		TxStatus::Connected { battery, noise_reduction: NoiseReduction::Off, unknown_b9: 0 }
	}

	fn state(tx1: TxStatus, tx2: TxStatus) -> Snapshot {
		Snapshot::Receiver(ReceiverState {
			mode: OutputMode::Split,
			gain: OutputGain::High,
			led: LedBrightness::Dim,
			tx1,
			tx2,
		})
	}

	fn texts(list: Vec<Announcement>) -> Vec<String> {
		list.into_iter().map(|a| a.text).collect()
	}

	#[test]
	fn status_text_lists_everything() {
		let text = status_text(&state(on(86), TxStatus::Disconnected));
		assert_eq!(
			text,
			"Receiver: connected\nOutput mode: Split\nOutput gain: High\nLED brightness: Dim\n\
			 Transmitter 1: connected, battery 86%, noise reduction Off\nTransmitter 2: not connected"
		);
		assert_eq!(status_text(&Snapshot::NoReceiver), NO_RECEIVER_TEXT);
	}

	#[test]
	fn summary_leaves_out_the_expected() {
		assert_eq!(summary(&Snapshot::NoReceiver), "Receiver not connected.");
		assert_eq!(
			summary(&state(on(86), on(87))),
			"Split mode. Transmitter 1, 86 percent. Transmitter 2, 87 percent."
		);
		assert_eq!(
			summary(&state(on(86), TxStatus::Disconnected)),
			"Split mode. Transmitter 1, 86 percent. Transmitter 2 not connected."
		);
		assert_eq!(
			summary(&state(TxStatus::Disconnected, TxStatus::Disconnected)),
			"Split mode. No transmitters connected."
		);
	}

	#[test]
	fn connection_changes_are_announced() {
		let mut a = Announcer::default();
		assert_eq!(texts(a.changes(&Snapshot::NoReceiver, &state(on(90), on(90)))), ["Receiver connected."]);
		assert_eq!(
			texts(a.changes(&state(on(90), on(90)), &state(on(90), TxStatus::Disconnected))),
			["Transmitter 2 disconnected."]
		);
		assert_eq!(
			texts(a.changes(&state(on(90), TxStatus::Disconnected), &state(on(90), on(90)))),
			["Transmitter 2 connected."]
		);
		assert_eq!(texts(a.changes(&state(on(90), on(90)), &Snapshot::NoReceiver)), ["Receiver disconnected."]);
	}

	#[test]
	fn the_first_reading_announces_no_connections() {
		let mut a = Announcer::default();
		assert!(a.changes(&Snapshot::Starting, &state(on(90), on(90))).is_empty());
	}

	#[test]
	fn battery_warns_once_per_level_and_resets_after_charging() {
		let mut a = Announcer::default();
		let step = |a: &mut Announcer, b: u8| texts(a.changes(&state(on(b), on(90)), &state(on(b), on(90))));
		assert!(step(&mut a, 21).is_empty());
		assert_eq!(step(&mut a, 20), ["Transmitter 1 battery 20 percent."]);
		assert!(step(&mut a, 21).is_empty(), "a wobble back up does not reset");
		assert!(step(&mut a, 19).is_empty());
		assert_eq!(step(&mut a, 10), ["Transmitter 1 battery 10 percent."]);
		assert!(step(&mut a, 9).is_empty());
		assert!(step(&mut a, 60).is_empty());
		assert_eq!(step(&mut a, 20), ["Transmitter 1 battery 20 percent."], "re-arms after charging");
	}

	#[test]
	fn a_low_battery_at_startup_warns_at_its_level() {
		let mut a = Announcer::default();
		assert_eq!(texts(a.changes(&Snapshot::Starting, &state(on(8), on(90)))), ["Transmitter 1 battery 8 percent."]);
	}
}
