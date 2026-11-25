//! Command-line tool for testing the receiver protocol.
//!
//! riteroad-cli status
//! riteroad-cli mode split|merged
//! riteroad-cli gain low|medium|high
//! riteroad-cli led dim|bright
//! riteroad-cli nr 1|2 off|low|high
//! riteroad-cli watch

use std::{process::ExitCode, thread::sleep, time::Duration};

use hidapi::HidApi;
use riteroad::*;

const USAGE: &str = "usage:
  riteroad-cli status
  riteroad-cli mode split|merged
  riteroad-cli gain low|medium|high
  riteroad-cli led dim|bright
  riteroad-cli nr 1|2 off|low|high
  riteroad-cli watch";

fn parse<T: Copy + std::fmt::Display>(all: &[T], arg: Option<&String>) -> Result<T, String> {
	let arg = arg.ok_or_else(|| USAGE.to_string())?;
	all.iter().copied().find(|v| v.to_string().eq_ignore_ascii_case(arg)).ok_or_else(|| {
		let names: Vec<String> = all.iter().map(|v| v.to_string().to_lowercase()).collect();
		format!("unknown value '{arg}', expected one of: {}", names.join(", "))
	})
}

fn parse_tx(arg: Option<&String>) -> Result<Transmitter, String> {
	match arg.map(String::as_str) {
		Some("1") => Ok(Transmitter::Tx1),
		Some("2") => Ok(Transmitter::Tx2),
		_ => Err("transmitter must be 1 or 2".into()),
	}
}

fn describe_tx(rx: &Receiver, tx: Transmitter) -> String {
	match rx.tx_status(tx) {
		Ok(TxStatus::Disconnected) => format!("{tx}: not connected"),
		Ok(TxStatus::Connected { battery, noise_reduction, unknown_b9 }) => format!(
			"{tx}: connected, battery {battery}%, noise reduction {noise_reduction} \
             (byte9=0x{unknown_b9:02x})"
		),
		Err(e) => format!("{tx}: error: {e}"),
	}
}

fn status(rx: &Receiver) -> Vec<String> {
	let show = |name: &str, r: Result<String, Error>| match r {
		Ok(v) => format!("{name}: {v}"),
		Err(e) => format!("{name}: error: {e}"),
	};
	vec![
		show("Output mode", rx.output_mode().map(|v| v.to_string())),
		show("Output gain", rx.output_gain().map(|v| v.to_string())),
		show("LED brightness", rx.led().map(|v| v.to_string())),
		describe_tx(rx, Transmitter::Tx1),
		describe_tx(rx, Transmitter::Tx2),
	]
}

fn run(args: &[String]) -> Result<(), String> {
	let api = HidApi::new().map_err(|e| e.to_string())?;
	let rx = Receiver::open(&api).map_err(|e| e.to_string())?;
	let err = |e: Error| e.to_string();

	match args.first().map(String::as_str) {
		Some("status") | None => {
			for line in status(&rx) {
				println!("{line}");
			}
		}
		Some("mode") => {
			let v = parse(OutputMode::ALL, args.get(1))?;
			rx.set_output_mode(v).map_err(err)?;
			println!("Output mode now reads: {}", rx.output_mode().map_err(err)?);
		}
		Some("gain") => {
			let v = parse(OutputGain::ALL, args.get(1))?;
			rx.set_output_gain(v).map_err(err)?;
			println!("Output gain now reads: {}", rx.output_gain().map_err(err)?);
		}
		Some("led") => {
			let v = parse(LedBrightness::ALL, args.get(1))?;
			rx.set_led(v).map_err(err)?;
			println!("LED brightness now reads: {}", rx.led().map_err(err)?);
		}
		Some("nr") => {
			let tx = parse_tx(args.get(1))?;
			let v = parse(NoiseReduction::ALL, args.get(2))?;
			rx.set_noise_reduction(tx, v).map_err(err)?;
			// The status reply can lag the write slightly.
			sleep(Duration::from_millis(300));
			println!("{}", describe_tx(&rx, tx));
		}
		Some("watch") => {
			println!("Watching for changes; press Ctrl+C to stop.");
			let mut last = Vec::new();
			loop {
				let now = status(&rx);
				for line in &now {
					if !last.contains(line) {
						println!("{line}");
					}
				}
				last = now;
				sleep(Duration::from_millis(500));
			}
		}
		Some(_) => return Err(USAGE.into()),
	}
	Ok(())
}

fn main() -> ExitCode {
	let args: Vec<String> = std::env::args().skip(1).collect();
	match run(&args) {
		Ok(()) => ExitCode::SUCCESS,
		Err(e) => {
			eprintln!("{e}");
			ExitCode::FAILURE
		}
	}
}
