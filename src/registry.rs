//! The few `HKEY_CURRENT_USER` registry calls riteRoad makes: the `Run` value and the Add/Remove
//! Programs entry.

use std::{io, iter, ptr};

use windows_sys::Win32::{
	Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
	System::Registry::{
		HKEY_CURRENT_USER, REG_DWORD, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegDeleteTreeW, RegGetValueW,
		RegSetKeyValueW,
	},
};

fn wide(text: &str) -> Vec<u16> {
	text.encode_utf16().chain(iter::once(0)).collect()
}

fn result(status: u32) -> io::Result<()> {
	if status == ERROR_SUCCESS { Ok(()) } else { Err(io::Error::from_raw_os_error(status.cast_signed())) }
}

/// Treats "it wasn't there" as success, for removals.
fn absent_is_fine(status: u32) -> io::Result<()> {
	result(if status == ERROR_FILE_NOT_FOUND { ERROR_SUCCESS } else { status })
}

pub fn set_string(key: &str, name: &str, value: &str) -> io::Result<()> {
	let (key, name, data) = (wide(key), wide(name), wide(value));
	let bytes = u32::try_from(data.len() * 2).map_err(io::Error::other)?;
	// SAFETY: every pointer is to a live, NUL-terminated UTF-16 buffer, and `bytes` is the data
	// buffer's length including its terminator.
	result(unsafe {
		RegSetKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), REG_SZ, data.as_ptr().cast(), bytes)
	})
}

pub fn set_dword(key: &str, name: &str, value: u32) -> io::Result<()> {
	let (key, name) = (wide(key), wide(name));
	// SAFETY: the key and name are live, NUL-terminated UTF-16 buffers, and the data is a live u32
	// of the size given.
	result(unsafe {
		RegSetKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), REG_DWORD, (&raw const value).cast(), 4)
	})
}

/// A string value, or `None` when it's missing or not a string.
pub fn get_string(key: &str, name: &str) -> Option<String> {
	let (key, name) = (wide(key), wide(name));
	let mut bytes = 0u32;
	// SAFETY: the key and name are live, NUL-terminated UTF-16 buffers. With no data buffer the call
	// only writes the size the value needs.
	let status = unsafe {
		RegGetValueW(
			HKEY_CURRENT_USER,
			key.as_ptr(),
			name.as_ptr(),
			RRF_RT_REG_SZ,
			ptr::null_mut(),
			ptr::null_mut(),
			&raw mut bytes,
		)
	};
	if status != ERROR_SUCCESS || bytes == 0 {
		return None;
	}
	let mut data = vec![0u16; (bytes as usize).div_ceil(2)];
	// SAFETY: as above, and `data` holds the `bytes` the value needs.
	let status = unsafe {
		RegGetValueW(
			HKEY_CURRENT_USER,
			key.as_ptr(),
			name.as_ptr(),
			RRF_RT_REG_SZ,
			ptr::null_mut(),
			data.as_mut_ptr().cast(),
			&raw mut bytes,
		)
	};
	if status != ERROR_SUCCESS {
		return None;
	}
	let end = data.iter().position(|&c| c == 0).unwrap_or(data.len());
	Some(String::from_utf16_lossy(&data[..end]))
}

pub fn delete_value(key: &str, name: &str) -> io::Result<()> {
	let (key, name) = (wide(key), wide(name));
	// SAFETY: both pointers are to live, NUL-terminated UTF-16 buffers.
	absent_is_fine(unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) })
}

/// Deletes `key` and everything under it.
pub fn delete_key(key: &str) -> io::Result<()> {
	let key = wide(key);
	// SAFETY: the pointer is to a live, NUL-terminated UTF-16 buffer.
	absent_is_fine(unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, key.as_ptr()) })
}
