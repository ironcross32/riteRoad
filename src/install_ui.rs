//! The install and uninstall dialogs. They only ask; `install` does the work.

use wx_utils::{add_ok_cancel_footer, build_ok_cancel_buttons_on, confirm, dialog_padding};
use wxdragon::prelude::*;

use crate::{app::APP_NAME, install};

/// Why the install dialog is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
	/// The Install to this PC button.
	Button,
	/// Start with Windows was ticked on a copy that isn't installed.
	StartWithWindows,
}

/// A dialog of `message`, the check boxes `checks` (label and starting value) and `ok_label` and
/// Cancel. Returns the boxes' values on OK.
fn ask(
	parent: &dyn WxWidget,
	title: &str,
	message: &str,
	checks: &[(&str, bool)],
	ok_label: &str,
) -> Option<Vec<bool>> {
	let dialog = Dialog::builder(parent, title).build();
	let padding = dialog_padding(&dialog);
	let panel = Panel::builder(&dialog).build();
	let content = BoxSizer::builder(Orientation::Vertical).build();
	let text = StaticText::builder(&panel).with_label(message).build();
	text.wrap(dialog.from_dip_int(420));
	content.add(&text, 0, SizerFlag::All, padding);
	let boxes: Vec<CheckBox> = checks
		.iter()
		.map(|&(label, value)| {
			let check = CheckBox::builder(&panel).with_label(label).build();
			check.set_value(value);
			content.add(&check, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom, padding);
			check
		})
		.collect();
	let (ok, cancel) = build_ok_cancel_buttons_on(&panel, &dialog, ok_label);
	add_ok_cancel_footer(content, ok, cancel);
	panel.set_sizer_and_fit(content, true);
	let outer = BoxSizer::builder(Orientation::Vertical).build();
	outer.add(&panel, 1, SizerFlag::Expand, 0);
	dialog.set_sizer_and_fit(outer, true);
	dialog.centre();
	match boxes.first() {
		Some(first) => first.set_focus(),
		None => ok.set_focus(),
	}
	let values = (dialog.show_modal() == ID_OK).then(|| boxes.iter().map(CheckBox::get_value).collect());
	dialog.destroy();
	values
}

fn folder() -> String {
	install::install_dir().map_or_else(String::new, |dir| dir.display().to_string())
}

/// Asks whether to install, and how. `None` means don't.
pub fn ask_install(parent: &dyn WxWidget, reason: Reason) -> Option<install::Options> {
	let lead = match reason {
		Reason::Button => "riteRoad will be installed to:",
		Reason::StartWithWindows => {
			"Start with Windows only works for a copy installed on this PC. Install riteRoad to:"
		}
	};
	let message = format!("{lead}\n{}\n\nThis copy will close, and the installed copy will open.", folder());
	let start_with_windows = reason == Reason::StartWithWindows || install::installed_copy_starts_with_windows();
	let values = ask(
		parent,
		"Install riteRoad",
		&message,
		&[("Create desktop shortcut", install::desktop_shortcut_exists()), ("Start with Windows", start_with_windows)],
		"Install",
	)?;
	if install::is_installed()
		&& !confirm(parent, "riteRoad is already installed. Replace it with this copy and its settings?", APP_NAME)
	{
		return None;
	}
	Some(install::Options { desktop_shortcut: values[0], start_with_windows: values[1] })
}

/// Asks whether to uninstall. `Some(delete_settings)` means go ahead.
pub fn ask_uninstall(parent: &dyn WxWidget, closes: bool) -> Option<bool> {
	let mut message = format!("riteRoad will be removed from this PC, from:\n{}", folder());
	if closes {
		message.push_str("\n\nriteRoad will close.");
	}
	ask(parent, "Uninstall riteRoad", &message, &[("Also delete settings", false)], "Uninstall").map(|values| values[0])
}
