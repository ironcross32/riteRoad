//! The settings file, `settings.toml` beside `riteroad.exe`, so the settings travel with the folder
//! it runs from. A missing or unreadable file means the defaults.
//!
//! Start with Windows isn't stored here: it's whether the `Run` value points at this copy, which
//! `startup` reads from the registry. Nothing in this file is specific to one PC.

use std::{env, fs, io, path::PathBuf};

use key_chord::KeyChord;
use serde::{Deserialize, Serialize};

pub const FILE_NAME: &str = "settings.toml";
/// Panics are appended here, beside the settings.
pub const PANIC_LOG: &str = "panic.log";
/// Written when the user has cleared the hotkey, so it stays cleared rather than falling back to
/// the default after a restart.
const NO_HOTKEY: &str = "none";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
	speak_status_hotkey: String,
	pub announce_receiver: bool,
	pub announce_transmitters: bool,
	pub announce_battery: bool,
	pub check_for_updates: bool,
}

impl Default for Config {
	fn default() -> Self {
		Self {
			speak_status_hotkey: default_hotkey().to_shortcut_string(),
			announce_receiver: true,
			announce_transmitters: true,
			announce_battery: true,
			check_for_updates: true,
		}
	}
}

impl Config {
	pub fn load() -> Self {
		path()
			.and_then(|path| fs::read_to_string(path).ok())
			.and_then(|text| toml::from_str(&text).ok())
			.unwrap_or_default()
	}

	pub fn save(&self) -> io::Result<()> {
		let path = path().ok_or_else(|| io::Error::other("couldn't find riteRoad's folder"))?;
		fs::write(path, toml::to_string(self).map_err(io::Error::other)?)
	}

	pub fn hotkey(&self) -> Option<KeyChord> {
		KeyChord::parse(&self.speak_status_hotkey)
	}

	pub fn set_hotkey(&mut self, chord: Option<&KeyChord>) {
		self.speak_status_hotkey = chord.map_or_else(|| NO_HOTKEY.into(), KeyChord::to_shortcut_string);
	}
}

/// Ctrl+Shift+F9 speaks the status from anywhere.
pub fn default_hotkey() -> KeyChord {
	KeyChord::new(true, false, true, "F9")
}

/// The folder `riteroad.exe` runs from.
pub fn app_dir() -> Option<PathBuf> {
	env::current_exe().ok()?.parent().map(PathBuf::from)
}

fn path() -> Option<PathBuf> {
	app_dir().map(|dir| dir.join(FILE_NAME))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn defaults_to_ctrl_shift_f9() {
		assert_eq!(Config::default().hotkey(), Some(default_hotkey()));
		assert_eq!(default_hotkey().to_shortcut_string(), "Ctrl+Shift+F9");
	}

	#[test]
	fn a_cleared_hotkey_stays_cleared() {
		let mut config = Config::default();
		config.set_hotkey(None);
		let reloaded: Config = toml::from_str(&toml::to_string(&config).unwrap()).unwrap();
		assert_eq!(reloaded.hotkey(), None);
	}

	#[test]
	fn an_empty_file_gives_the_defaults() {
		assert_eq!(toml::from_str::<Config>("").unwrap(), Config::default());
	}

	#[test]
	fn an_old_file_with_start_with_windows_still_loads() {
		let config: Config = toml::from_str("start_with_windows = true\nannounce_battery = false\n").unwrap();
		assert!(!config.announce_battery);
		assert!(config.check_for_updates);
	}
}
