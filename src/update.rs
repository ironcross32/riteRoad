//! Checking GitHub for a newer riteRoad, and installing it, through ship-shape.
//!
//! Every copy, installed or not, updates as a portable one: ship-shape unzips the release over the
//! folder `riteroad.exe` runs from and starts it again. `settings.toml` is never in the zip, so the
//! settings survive.

use std::{
	fs,
	sync::{
		Arc,
		atomic::{AtomicBool, Ordering},
	},
};

use ship_shape::{
	InstallKind, UpdateChannel, UpdaterConfig,
	ui::{self, CheckTrigger},
};
use wx_utils::show_error;
use wxdragon::prelude::*;

use crate::{app::APP_NAME, config};

const REPOSITORY: &str = "ironcross32/riteRoad";
/// The minisign key riteRoad's releases are signed with. An update whose signature it doesn't
/// verify is never installed.
const PUBLIC_KEY: &str = "RWRUOKErwboM41iAVZa111p/oaKwxJOv46aSsvY7EtZBk/clR3myKkMw";

/// Looks for a newer release, with its dialogs over `parent`. An automatic check only speaks up
/// when there's one; a check the user asked for also says when riteRoad is up to date, or why it
/// couldn't tell.
///
/// Once an update is downloaded, ship-shape sets `exit`, and riteRoad shuts down the way every
/// Exit does (see `app`). ship-shape ends the process itself if that takes over 15 seconds.
pub fn check(parent: &dyn WxWidget, automatic: bool, exit: &Arc<AtomicBool>) {
	if !folder_is_writable() {
		// Unattended, there's nobody to tell, and saying so at every sign-in would be noise.
		if !automatic {
			let folder = config::app_dir().map_or_else(String::new, |dir| format!(" ({})", dir.display()));
			show_error(
				parent,
				format!("riteRoad can't update itself, because it can't write to the folder it runs from{folder}."),
				APP_NAME,
			);
		}
		return;
	}
	let updater = Arc::new(
		UpdaterConfig::new(REPOSITORY, "riteroad", APP_NAME, PUBLIC_KEY, env!("CARGO_PKG_VERSION"))
			.with_install_kind(InstallKind::Portable)
			.with_asset_suffix(if cfg!(target_arch = "aarch64") { "-arm64" } else { "-x64" }),
	);
	let trigger = if automatic { CheckTrigger::Automatic } else { CheckTrigger::Manual };
	let exit = Arc::clone(exit);
	ui::run_update_check_with_exit_handler(updater, parent, UpdateChannel::Stable, trigger, move || {
		exit.store(true, Ordering::SeqCst);
	});
}

/// Whether ship-shape could unzip an update into the folder riteRoad runs from.
fn folder_is_writable() -> bool {
	let Some(dir) = config::app_dir() else {
		return false;
	};
	let probe = dir.join(".riteroad-write-test");
	let writable = fs::OpenOptions::new().write(true).create_new(true).open(&probe).is_ok();
	let _ = fs::remove_file(probe);
	writable
}

/// Lets a copy that an update relaunched come to the front, which Windows would otherwise open
/// behind whatever the user was doing. Called once the window is first shown.
pub fn come_forward_after_update(frame: &Frame) {
	ui::bring_to_front_after_update(frame);
}
