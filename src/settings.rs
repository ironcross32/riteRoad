//! The Settings dialog: Hotkey, Automatic announcements and Startup groups, then OK and Cancel.
//! Nothing changes until OK.

use std::{cell::RefCell, rc::Rc};

use key_chord::KeyChord;
use wx_utils::{add_ok_cancel_footer, build_ok_cancel_buttons_on, dialog_padding};
use wxdragon::prelude::*;

use crate::{config::Config, speech};

/// Keys the hotkey field lets through rather than captures: Tab and Shift+Tab move focus, Escape
/// cancels the dialog, and the arrows keep the screen reader's own navigation working.
const PASS_THROUGH: [i32; 6] = [WXK_TAB, WXK_ESCAPE, 314, 315, 316, 317];
const CLEAR_KEYS: [i32; 3] = [WXK_DELETE, WXK_BACK, WXK_NUMPAD_DELETE];

fn describe(chord: Option<&KeyChord>) -> String {
	chord.map_or_else(|| "None".into(), KeyChord::to_display_string)
}

/// A group box whose controls are created inside it, so screen readers report the group.
fn group(panel: &Panel, label: &str) -> (StaticBoxSizer, StaticBox) {
	let sizer = StaticBoxSizerBuilder::new_with_label(Orientation::Vertical, panel, label).build();
	let static_box = sizer.get_static_box().expect("a labelled static box sizer has a box");
	(sizer, static_box)
}

/// Shows the dialog. Returns the edited settings on OK, or `None` on Cancel.
pub fn show(parent: &dyn WxWidget, current: &Config) -> Option<Config> {
	let dialog = Dialog::builder(parent, "riteRoad Settings").build();
	let padding = dialog_padding(&dialog);
	let panel = Panel::builder(&dialog).build();
	let content = BoxSizer::builder(Orientation::Vertical).build();

	// Hotkey
	let (hotkey_sizer, hotkey_box) = group(&panel, "Hotkey");
	let hotkey_label = StaticText::builder(&hotkey_box).with_label("Speak status hotkey:").build();
	// Multi-line because wxWidgets leaves read-only single-line fields out of the Tab order, and
	// this one must be reachable with Tab. It only ever holds one line.
	let hotkey_field = TextCtrl::builder(&hotkey_box)
		.with_style(TextCtrlStyle::ReadOnly | TextCtrlStyle::MultiLine | TextCtrlStyle::DontWrap)
		.build();
	hotkey_field.set_name("Speak status hotkey");
	let hint = StaticText::builder(&hotkey_box)
		.with_label(
			"Press the keys you want while this field has focus. Delete or Backspace clears it. If nothing changes \
			 when you press a combination, another program is already using it.",
		)
		.build();
	hotkey_sizer.add(&hotkey_label, 0, SizerFlag::All, padding / 2);
	hotkey_sizer.add(&hotkey_field, 0, SizerFlag::Expand | SizerFlag::All, padding / 2);
	hotkey_sizer.add(&hint, 0, SizerFlag::All, padding / 2);

	let chord = Rc::new(RefCell::new(current.hotkey()));
	hotkey_field.set_value(&describe(chord.borrow().as_ref()));
	hotkey_field.on_key_down({
		let chord = Rc::clone(&chord);
		move |event| {
			let WindowEventData::Keyboard(ref key) = event else {
				event.skip(true);
				return;
			};
			let Some(code) = key.get_key_code() else {
				event.skip(true);
				return;
			};
			let (ctrl, alt, shift) = (key.control_down(), key.alt_down(), key.shift_down());
			let plain = !ctrl && !alt;
			if PASS_THROUGH.contains(&code) {
				event.skip(true);
				return;
			}
			// A multi-line field keeps Enter for itself, so press OK on its behalf.
			if code == WXK_RETURN && plain {
				dialog.end_modal(ID_OK);
				event.skip(false);
				return;
			}
			if CLEAR_KEYS.contains(&code) && plain && !shift {
				*chord.borrow_mut() = None;
				hotkey_field.set_value(&describe(None));
				speech::output("Hotkey cleared.");
				event.skip(false);
				return;
			}
			// None for a bare modifier, pressed on the way to a chord.
			let Some(pressed) = KeyChord::from_key_code(code, ctrl, alt, shift) else {
				event.skip(true);
				return;
			};
			if plain {
				speech::output("A hotkey must include Ctrl or Alt.");
			} else {
				let text = pressed.to_display_string();
				hotkey_field.set_value(&text);
				speech::output(&text);
				*chord.borrow_mut() = Some(pressed);
			}
			event.skip(false);
		}
	});
	content.add_sizer(&hotkey_sizer, 0, SizerFlag::Expand | SizerFlag::All, padding);

	// Automatic announcements
	let (announce_sizer, announce_box) = group(&panel, "Automatic announcements");
	let checkbox = |label: &str, value: bool| {
		let check = CheckBox::builder(&announce_box).with_label(label).build();
		check.set_value(value);
		announce_sizer.add(&check, 0, SizerFlag::All, padding / 2);
		check
	};
	let receiver = checkbox("Receiver connected or disconnected", current.announce_receiver);
	let transmitters = checkbox("Transmitter connected or disconnected", current.announce_transmitters);
	let battery = checkbox("Low battery warnings", current.announce_battery);
	content.add_sizer(&announce_sizer, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right, padding);

	// Startup
	let (startup_sizer, startup_box) = group(&panel, "Startup");
	let start_with_windows = CheckBox::builder(&startup_box).with_label("Start with Windows").build();
	start_with_windows.set_value(current.start_with_windows);
	startup_sizer.add(&start_with_windows, 0, SizerFlag::All, padding / 2);
	content.add_sizer(&startup_sizer, 0, SizerFlag::Expand | SizerFlag::All, padding);

	let (ok, cancel) = build_ok_cancel_buttons_on(&panel, &dialog, "OK");
	add_ok_cancel_footer(content, ok, cancel);
	panel.set_sizer_and_fit(content, true);
	let outer = BoxSizer::builder(Orientation::Vertical).build();
	outer.add(&panel, 1, SizerFlag::Expand, 0);
	dialog.set_sizer_and_fit(outer, true);
	dialog.centre();
	hotkey_field.set_focus();

	let result = dialog.show_modal();
	let edited = (result == ID_OK).then(|| {
		let mut config = current.clone();
		config.set_hotkey(chord.borrow().as_ref());
		config.announce_receiver = receiver.get_value();
		config.announce_transmitters = transmitters.get_value();
		config.announce_battery = battery.get_value();
		config.start_with_windows = start_with_windows.get_value();
		config
	});
	dialog.destroy();
	edited
}
