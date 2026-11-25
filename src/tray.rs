//! The notification-area icon, with Show riteRoad, Settings and Exit.
//!
//! Modelled on `wx_utils::tray::Tray`, whose menu has no room for Settings. The handlers given
//! here run inside the icon's own menu or click handler, so they must not destroy the icon or the
//! window: removing the icon from inside its own handler returns into freed objects and Windows
//! ends the process with 0xc000041d. The app only sets flags from them, and acts on those from its
//! timer once the handler has returned.

use std::cell::RefCell;

use wxdragon::prelude::*;

const ID_SHOW: i32 = ID_HIGHEST + 4900;
const ID_SETTINGS: i32 = ID_HIGHEST + 4901;
const ID_EXIT: i32 = ID_HIGHEST + 4902;

pub struct Tray {
	icon: TaskBarIcon,
	/// Kept for as long as the icon shows it; `None` once removed.
	menu: RefCell<Option<Menu>>,
}

pub struct Handlers {
	pub show: Box<dyn Fn()>,
	pub settings: Box<dyn Fn()>,
	pub exit: Box<dyn Fn()>,
}

impl Tray {
	pub fn new(tooltip: &str, handlers: Handlers) -> Self {
		let mut menu = Menu::builder()
			.append_item(ID_SHOW, "&Show riteRoad", "Show the riteRoad window")
			.append_item(ID_SETTINGS, "S&ettings", "Change riteRoad's settings")
			.append_separator()
			.append_item(ID_EXIT, "E&xit", "Close riteRoad")
			.build();
		let icon = TaskBarIcon::builder().build();
		icon.set_popup_menu(&mut menu);
		let Handlers { show, settings, exit } = handlers;
		let show = std::rc::Rc::new(show);
		let click = std::rc::Rc::clone(&show);
		icon.on_menu(move |event| match event.get_id() {
			ID_SHOW => show(),
			ID_SETTINGS => settings(),
			ID_EXIT => exit(),
			_ => {}
		});
		// Windows reports a click as the button coming up.
		icon.on_left_up(move |_| click());
		if let Some(bitmap) = ArtProvider::get_bitmap(ArtId::Information, ArtClient::Menu, Some(Size::new(16, 16))) {
			icon.set_icon(&bitmap, tooltip);
		}
		Self { icon, menu: RefCell::new(Some(menu)) }
	}

	/// Takes the icon out of the notification area. Never call it from the icon's own handlers.
	/// Safe to call more than once.
	pub fn remove(&self) {
		if let Some(mut menu) = self.menu.borrow_mut().take() {
			menu.destroy_menu();
			self.icon.destroy();
		}
	}
}
