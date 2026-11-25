#![cfg_attr(not(test), windows_subsystem = "windows")]

mod app;
mod config;
mod install;
mod install_ui;
mod poller;
mod registry;
mod settings;
mod speech;
mod startup;
mod status;
mod tray;
mod update;

use std::{env, sync::mpsc};

use doorbell::{Ring, ring};

/// Identifies the running copy to doorbell. Keep it stable across releases.
const INSTANCE_ID: &str = "riteroad";

/// Appends panics to `panic.log` beside the executable, since a windowed app has no console to
/// print them to.
fn log_panics() {
	let default = std::panic::take_hook();
	std::panic::set_hook(Box::new(move |info| {
		if let Some(path) = config::app_dir().map(|dir| dir.join(config::PANIC_LOG))
			&& let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path)
		{
			use std::io::Write;
			let backtrace = std::backtrace::Backtrace::force_capture();
			let _ = writeln!(file, "{info}\n{backtrace}\n");
		}
		default(info);
	}));
}

fn main() {
	log_panics();
	let args: Vec<String> = env::args().skip(1).collect();
	// A second launch only passes its arguments to the running copy, which comes to the front, or
	// for `--uninstall`, offers to uninstall.
	let (message_tx, messages) = mpsc::channel();
	match ring(INSTANCE_ID, &args) {
		Ok(Ring::Delivered) => return,
		Ok(Ring::First(door)) => door.listen(move |message| {
			let _ = message_tx.send(message);
		}),
		// Not being able to check for another copy shouldn't stop this one.
		Err(_) => (),
	}
	if args.iter().any(|arg| arg == install::UNINSTALL_ARG) {
		let _ = wxdragon::main(|_| app::uninstall_alone());
		return;
	}
	let start_hidden = args.iter().any(|arg| arg == startup::TRAY_ARG);
	let poller = poller::spawn();
	let _ = wxdragon::main(move |_| {
		app::run(app::Channels {
			latest: poller.latest,
			commands: poller.commands,
			failures: poller.failures,
			messages,
			start_hidden,
		});
	});
}
