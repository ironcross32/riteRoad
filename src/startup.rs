//! Starting riteRoad with Windows, through the current user's `Run` registry key.

use std::{env, io, iter};

use windows_sys::Win32::{
	Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
	System::Registry::{HKEY_CURRENT_USER, REG_SZ, RegDeleteKeyValueW, RegSetKeyValueW},
};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "riteRoad";
/// Passed when Windows starts riteRoad, so it goes straight to the tray.
pub const TRAY_ARG: &str = "--tray";

fn wide(text: &str) -> Vec<u16> {
	text.encode_utf16().chain(iter::once(0)).collect()
}

/// Adds or removes the entry that starts this executable when the user signs in.
pub fn set_enabled(enabled: bool) -> io::Result<()> {
	let key = wide(RUN_KEY);
	let name = wide(VALUE_NAME);
	let status = if enabled {
		let command = format!("\"{}\" {TRAY_ARG}", env::current_exe()?.display());
		let data = wide(&command);
		let bytes = u32::try_from(data.len() * 2).map_err(io::Error::other)?;
		// SAFETY: every pointer is to a live, NUL-terminated UTF-16 buffer, and `bytes` is the
		// data buffer's length including its terminator.
		unsafe { RegSetKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), REG_SZ, data.as_ptr().cast(), bytes) }
	} else {
		// SAFETY: both pointers are to live, NUL-terminated UTF-16 buffers.
		match unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) } {
			ERROR_FILE_NOT_FOUND => ERROR_SUCCESS,
			other => other,
		}
	};
	if status == ERROR_SUCCESS { Ok(()) } else { Err(io::Error::from_raw_os_error(status.cast_signed())) }
}
