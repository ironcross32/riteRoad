//! Starting riteRoad with Windows, through the current user's `Run` registry key.
//!
//! Only the installed copy is ever registered (see `install`), so moving a portable folder can't
//! leave a `Run` value pointing nowhere. Whether startup is on is read from the registry each time,
//! never stored in the settings, so a folder carried to another PC doesn't claim it.

use std::{
	env, io,
	path::{Path, PathBuf},
};

use crate::registry;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "riteRoad";
/// Passed when Windows starts riteRoad, so it goes straight to the tray.
pub const TRAY_ARG: &str = "--tray";

/// The command Windows runs at sign-in to start `exe`.
fn command_for(exe: &Path) -> String {
	format!("\"{}\" {TRAY_ARG}", exe.display())
}

/// The executable at the start of a command line: the quoted part, or up to the first space.
fn exe_in_command(command: &str) -> Option<PathBuf> {
	let command = command.trim();
	let exe = match command.strip_prefix('"') {
		Some(rest) => rest.split('"').next()?,
		None => command.split(' ').next()?,
	};
	(!exe.is_empty()).then(|| PathBuf::from(exe))
}

pub fn same_file(a: &Path, b: &Path) -> bool {
	match (a.canonicalize(), b.canonicalize()) {
		(Ok(a), Ok(b)) => a == b,
		_ => a.as_os_str().eq_ignore_ascii_case(b.as_os_str()),
	}
}

/// Whether Windows starts `exe` at sign-in.
pub fn starts(exe: &Path) -> bool {
	registry::get_string(RUN_KEY, VALUE_NAME)
		.and_then(|command| exe_in_command(&command))
		.is_some_and(|registered| same_file(&registered, exe))
}

/// Whether Windows starts this copy at sign-in.
pub fn is_enabled() -> bool {
	env::current_exe().is_ok_and(|exe| starts(&exe))
}

/// Has Windows start `exe` at sign-in, replacing any earlier entry.
pub fn enable_for(exe: &Path) -> io::Result<()> {
	registry::set_string(RUN_KEY, VALUE_NAME, &command_for(exe))
}

/// Removes the entry, whichever copy it starts.
pub fn disable() -> io::Result<()> {
	registry::delete_value(RUN_KEY, VALUE_NAME)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn reads_the_exe_back_out_of_its_command() {
		let exe = Path::new(r"C:\Users\Some One\AppData\Local\Programs\riteRoad\riteroad.exe");
		assert_eq!(exe_in_command(&command_for(exe)).as_deref(), Some(exe));
	}

	#[test]
	fn an_unquoted_command_ends_at_the_first_space() {
		assert_eq!(
			exe_in_command(r"C:\riteRoad\riteroad.exe --tray").as_deref(),
			Some(Path::new(r"C:\riteRoad\riteroad.exe"))
		);
		assert_eq!(exe_in_command("  "), None);
	}
}
