#![cfg_attr(not(test), windows_subsystem = "windows")]

mod app;
mod config;
mod poller;
mod settings;
mod speech;
mod startup;
mod status;
mod tray;

use std::{env, sync::mpsc};

use doorbell::{Ring, ring};

/// Identifies the running copy to doorbell. Keep it stable across releases.
const INSTANCE_ID: &str = "riteroad";

/// Appends panics to `%LOCALAPPDATA%\riteRoad\panic.log`, since a windowed app has no console to
/// print them to.
fn log_panics() {
	let default = std::panic::take_hook();
	std::panic::set_hook(Box::new(move |info| {
		if let Some(dir) = env::var_os("LOCALAPPDATA").map(|d| std::path::PathBuf::from(d).join("riteRoad")) {
			let _ = std::fs::create_dir_all(&dir);
			if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("panic.log")) {
				use std::io::Write;
				let backtrace = std::backtrace::Backtrace::force_capture();
				let _ = writeln!(file, "{info}\n{backtrace}\n");
			}
		}
		default(info);
	}));
}

fn main() {
	log_panics();
	// A second launch only asks the running copy to come to the front.
	let (raise_tx, raise) = mpsc::channel();
	match ring(INSTANCE_ID, &[] as &[&str]) {
		Ok(Ring::Delivered) => return,
		Ok(Ring::First(door)) => door.listen(move |_| {
			let _ = raise_tx.send(());
		}),
		// Not being able to check for another copy shouldn't stop this one.
		Err(_) => (),
	}
	let start_hidden = env::args().any(|arg| arg == startup::TRAY_ARG);
	let poller = poller::spawn();
	let _ = wxdragon::main(move |_| {
		app::run(app::Channels {
			latest: poller.latest,
			commands: poller.commands,
			failures: poller.failures,
			raise,
			start_hidden,
		});
	});
}
