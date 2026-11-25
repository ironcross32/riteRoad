//! The main window, its menus, the tray icon and the speak-status hotkey.
//!
//! Shutdown: File > Exit, the Exit button and the tray's Exit only set `exiting`. The UI timer
//! sees it on its next tick, once whatever menu or handler set it has returned, waits for any
//! open dialog to close, then closes the frame. The frame's close handler is the single place
//! that stops the timer and the poller, lets go of the hotkey and removes the tray icon, before
//! wxWidgets destroys the window. Closing the window any other way (X, Alt+F4) is vetoed and
//! hides it to the tray instead.

use std::{
	cell::{Cell, OnceCell, RefCell},
	mem,
	rc::{Rc, Weak},
	sync::mpsc::{Receiver, Sender},
	time::{Duration, Instant},
};

use key_chord::KeyChord;
use riteroad::{LedBrightness, NoiseReduction, OutputGain, OutputMode, Transmitter, TxStatus};
use wx_utils::{
	AboutBoxBuilder, global_hotkeys::GlobalHotkeys, post_to_ui, show_error, show_warning, tray::Visibility,
};
use wxdragon::{prelude::*, timer::Timer};

use crate::{
	config::Config,
	poller::{Command, Latest, read_latest},
	settings, speech, startup,
	status::{Announcer, Category, ReceiverState, Snapshot, status_text, summary},
	tray::{Handlers, Tray},
};

pub const APP_NAME: &str = "riteRoad";
const UI_TICK_MS: i32 = 200;
/// After the user changes a setting, how long readings taken before the change reached the
/// receiver are kept from moving the control back.
const HOLD_AFTER_CHANGE: Duration = Duration::from_millis(1500);

const ID_SETTINGS: i32 = ID_HIGHEST + 100;
const ID_EXIT: i32 = ID_HIGHEST + 101;
const ID_ABOUT: i32 = ID_HIGHEST + 102;

const NR_TARGETS: [&str; 3] = ["Both", "Transmitter 1", "Transmitter 2"];
const MIXED: &str = "Mixed";

pub struct Channels {
	pub latest: Latest,
	pub commands: Sender<Command>,
	/// Why a change failed, to speak.
	pub failures: Receiver<String>,
	/// A second copy was started, so come to the front.
	pub raise: Receiver<()>,
	/// Started by Windows at sign-in: stay in the tray.
	pub start_hidden: bool,
}

struct Controls {
	frame: Frame,
	panel: Panel,
	status: TextCtrl,
	mode: Choice,
	gain: Choice,
	led: Choice,
	nr_target: Choice,
	nr_level: Choice,
	exit: Button,
}

struct App {
	frame: Frame,
	status: TextCtrl,
	mode: Choice,
	gain: Choice,
	led: Choice,
	nr_target: Choice,
	nr_level: Choice,
	visibility: Rc<Visibility>,
	tray: OnceCell<Tray>,
	/// Exit was asked for; the timer closes the frame on its next tick.
	exiting: Cell<bool>,
	/// The tray asked for Settings; the timer opens it once the tray menu has returned.
	settings_requested: Cell<bool>,
	settings_open: Cell<bool>,
	config: RefCell<Config>,
	hotkeys: RefCell<Option<GlobalHotkeys>>,
	latest: Latest,
	commands: Sender<Command>,
	/// The snapshot the window last showed.
	shown: Cell<Snapshot>,
	announcer: RefCell<Announcer>,
	/// Until when readings may not move the settings controls.
	hold_until: Cell<Option<Instant>>,
	/// Whether the Level dropdown currently has its "Mixed" item.
	level_mixed: Cell<bool>,
}

pub fn run(channels: Channels) {
	let Channels { latest, commands, failures, raise, start_hidden } = channels;
	speech::init();
	let Controls { frame, panel, status, mode, gain, led, nr_target, nr_level, exit } = lay_out();
	let app = Rc::new(App {
		frame,
		status,
		mode,
		gain,
		led,
		nr_target,
		nr_level,
		visibility: Rc::new(Visibility::new(frame)),
		tray: OnceCell::new(),
		exiting: Cell::new(false),
		settings_requested: Cell::new(false),
		settings_open: Cell::new(false),
		config: RefCell::new(Config::load()),
		hotkeys: RefCell::new(None),
		latest,
		commands,
		shown: Cell::new(Snapshot::Starting),
		announcer: RefCell::new(Announcer::default()),
		hold_until: Cell::new(None),
		level_mixed: Cell::new(false),
	});

	let tray = Tray::new(
		APP_NAME,
		Handlers {
			show: Box::new({
				let app = Rc::downgrade(&app);
				move || with(&app, |app| app.visibility.come_forward())
			}),
			settings: Box::new({
				let app = Rc::downgrade(&app);
				move || with(&app, |app| app.settings_requested.set(true))
			}),
			exit: Box::new({
				let app = Rc::downgrade(&app);
				move || with(&app, |app| app.exiting.set(true))
			}),
		},
	);
	let _ = app.tray.set(tray);

	app.frame.on_menu({
		let app = Rc::downgrade(&app);
		move |event| {
			with(&app, |app| match event.get_id() {
				ID_SETTINGS => app.open_settings(),
				ID_EXIT => app.exiting.set(true),
				ID_ABOUT => app.about(),
				_ => {}
			});
		}
	});
	exit.on_click({
		let app = Rc::downgrade(&app);
		move |_| with(&app, |app| app.exiting.set(true))
	});
	bind_choice(&app, app.mode, |app, index| {
		let mode = *OutputMode::ALL.get(index)?;
		Some(Command::SetMode(mode)).inspect(|_| app.hold())
	});
	bind_choice(&app, app.gain, |app, index| {
		let gain = *OutputGain::ALL.get(index)?;
		Some(Command::SetGain(gain)).inspect(|_| app.hold())
	});
	bind_choice(&app, app.led, |app, index| {
		let led = *LedBrightness::ALL.get(index)?;
		Some(Command::SetLed(led)).inspect(|_| app.hold())
	});
	bind_choice(&app, app.nr_level, |app, index| {
		// "Mixed" only describes the transmitters; choosing it changes nothing.
		let level = *NoiseReduction::ALL.get(index)?;
		let Snapshot::Receiver(state) = app.shown.get() else {
			return None;
		};
		let targets: Vec<Transmitter> =
			app.targets().into_iter().filter(|&tx| state.tx(tx) != TxStatus::Disconnected).collect();
		if targets.is_empty() {
			return None;
		}
		app.hold();
		Some(Command::SetNoiseReduction(targets, level))
	});
	app.nr_target.on_selection_changed({
		let app = Rc::downgrade(&app);
		move |_| with(&app, |app| app.show_level(&app.shown.get()))
	});

	let timer = Timer::new(&panel);
	timer.on_tick({
		let app = Rc::downgrade(&app);
		move |_| with(&app, |app| app.tick(&raise, &failures))
	});
	timer.start(UI_TICK_MS, false);

	app.frame.on_close({
		let app = Rc::clone(&app);
		move |event| {
			if !app.exiting.get() && veto(&event) {
				app.visibility.hide();
				return;
			}
			timer.stop();
			let _ = app.commands.send(Command::Stop);
			// Leaked rather than dropped: dropping joins the hotkey thread, which can hang the
			// window while it closes. Windows releases the hotkey when the process exits.
			mem::forget(app.hotkeys.borrow_mut().take());
			if let Some(tray) = app.tray.get() {
				tray.remove();
			}
			event.skip(true);
		}
	});

	app.refresh();
	if let Some(chord) = app.config.borrow().hotkey()
		&& !app.register_hotkey(Some(&chord))
	{
		app.warn_unregistered(&chord);
	}
	app.frame.centre();
	if !start_hidden {
		app.visibility.come_forward();
		app.status.set_focus();
	}
}

/// Runs `f` on the app if it still exists, so handlers don't keep it alive.
fn with(app: &Weak<App>, f: impl FnOnce(&App)) {
	if let Some(app) = app.upgrade() {
		f(&app);
	}
}

/// Sends the command `make` builds when the user picks an item in `choice`.
fn bind_choice(app: &Rc<App>, choice: Choice, make: impl Fn(&App, usize) -> Option<Command> + 'static) {
	let app = Rc::downgrade(app);
	choice.on_selection_changed(move |_| {
		with(&app, |app| {
			if let Some(index) = choice.get_selection()
				&& let Some(command) = make(app, index as usize)
			{
				let _ = app.commands.send(command);
			}
		});
	});
}

/// Vetoes a close event if it can be. Returns whether it was.
fn veto(event: &WindowEventData) -> bool {
	let event = match event {
		WindowEventData::Session(session) => &session.event,
		WindowEventData::General(event) => event,
		_ => return false,
	};
	if event.can_veto() {
		event.veto();
		true
	} else {
		false
	}
}

fn labels<T: std::fmt::Display>(all: &[T]) -> Vec<String> {
	all.iter().map(ToString::to_string).collect()
}

/// A labelled dropdown in `parent`, added to `sizer` as one row.
fn dropdown(parent: &StaticBox, sizer: &StaticBoxSizer, label: &str, items: Vec<String>) -> Choice {
	let row = BoxSizer::builder(Orientation::Horizontal).build();
	let text = StaticText::builder(parent).with_label(&format!("{label}:")).build();
	let choice = Choice::builder(parent).with_choices(items).build();
	choice.set_name(label);
	row.add(&text, 0, SizerFlag::AlignCenterVertical | SizerFlag::Right, 8);
	row.add(&choice, 1, SizerFlag::Expand, 0);
	sizer.add_sizer(&row, 0, SizerFlag::Expand | SizerFlag::All, 6);
	choice
}

fn group(panel: &Panel, label: &str) -> (StaticBoxSizer, StaticBox) {
	let sizer = StaticBoxSizerBuilder::new_with_label(Orientation::Vertical, panel, label).build();
	let static_box = sizer.get_static_box().expect("a labelled static box sizer has a box");
	(sizer, static_box)
}

fn lay_out() -> Controls {
	let frame = Frame::builder().with_title(&format!("{APP_NAME} - RØDE Wireless Micro")).build();
	let file = Menu::builder()
		.append_item(ID_SETTINGS, "&Settings…", "Change riteRoad's settings")
		.append_separator()
		.append_item(ID_EXIT, "E&xit", "Close riteRoad")
		.build();
	let help = Menu::builder().append_item(ID_ABOUT, "&About riteRoad", "About riteRoad").build();
	frame.set_menu_bar(MenuBar::builder().append(file, "&File").append(help, "&Help").build());

	let panel = Panel::builder(&frame).build();
	let sizer = BoxSizer::builder(Orientation::Vertical).build();
	let status_label = StaticText::builder(&panel).with_label("Status:").build();
	let status = TextCtrl::builder(&panel)
		.with_style(TextCtrlStyle::ReadOnly | TextCtrlStyle::MultiLine)
		.with_size(Size::new(460, 130))
		.build();
	status.set_name("Status");
	sizer.add(&status_label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 12);
	sizer.add(&status, 1, SizerFlag::Expand | SizerFlag::All, 12);

	let (receiver_sizer, receiver_box) = group(&panel, "Receiver");
	let mode = dropdown(&receiver_box, &receiver_sizer, "Output mode", labels(OutputMode::ALL));
	let gain = dropdown(&receiver_box, &receiver_sizer, "Output gain", labels(OutputGain::ALL));
	let led = dropdown(&receiver_box, &receiver_sizer, "LED brightness", labels(LedBrightness::ALL));
	sizer.add_sizer(&receiver_sizer, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right, 12);

	let (nr_sizer, nr_box) = group(&panel, "Noise reduction");
	let nr_target = dropdown(&nr_box, &nr_sizer, "Transmitter", NR_TARGETS.map(String::from).to_vec());
	nr_target.set_selection(0);
	let nr_level = dropdown(&nr_box, &nr_sizer, "Level", labels(NoiseReduction::ALL));
	sizer.add_sizer(&nr_sizer, 0, SizerFlag::Expand | SizerFlag::All, 12);

	let exit = Button::builder(&panel).with_label("E&xit").build();
	sizer.add(&exit, 0, SizerFlag::AlignRight | SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom, 12);

	panel.set_sizer_and_fit(sizer, true);
	let outer = BoxSizer::builder(Orientation::Vertical).build();
	outer.add(&panel, 1, SizerFlag::Expand, 0);
	frame.set_sizer_and_fit(outer, true);
	Controls { frame, panel, status, mode, gain, led, nr_target, nr_level, exit }
}

/// Selects `index` in `choice` unless it already is, so an unchanged reading doesn't disturb a
/// screen reader that is on the control.
fn select(choice: Choice, index: usize) {
	let index = u32::try_from(index).unwrap_or(0);
	if choice.get_selection() != Some(index) {
		choice.set_selection(index);
	}
}

fn index_of<T: PartialEq>(all: &[T], value: T) -> usize {
	all.iter().position(|v| *v == value).unwrap_or(0)
}

impl App {
	fn tick(&self, raise: &Receiver<()>, failures: &Receiver<String>) {
		if self.exiting.get() {
			// Closing the frame under a modal dialog upsets wxWidgets, so ask the dialog to close
			// first and come back on a later tick.
			if !self.visibility.close_dialog() {
				self.frame.close(true);
			}
			return;
		}
		if raise.try_iter().count() > 0 {
			self.visibility.come_forward();
		}
		if self.settings_requested.replace(false) {
			self.visibility.come_forward();
			self.open_settings();
			return;
		}
		if let Some(message) = failures.try_iter().last() {
			speech::output(&message);
		}
		self.refresh();
	}

	/// Shows the latest reading, and announces what changed.
	fn refresh(&self) {
		let current = read_latest(&self.latest);
		let previous = self.shown.replace(current);
		if current != previous {
			let changes = self.announcer.borrow_mut().changes(&previous, &current);
			let config = self.config.borrow();
			let spoken: Vec<String> = changes
				.into_iter()
				.filter(|a| match a.category {
					Category::Receiver => config.announce_receiver,
					Category::Transmitter => config.announce_transmitters,
					Category::Battery => config.announce_battery,
				})
				.map(|a| a.text)
				.collect();
			if !spoken.is_empty() {
				// One utterance, since each interrupts the one before.
				speech::output(&spoken.join(" "));
			}
		}
		self.set_status(&status_text(&current));
		let present = matches!(current, Snapshot::Receiver(_));
		for choice in [self.mode, self.gain, self.led, self.nr_target] {
			choice.enable(present);
		}
		let held = self.hold_until.get().is_some_and(|until| Instant::now() < until);
		if let Snapshot::Receiver(state) = current
			&& !held
		{
			self.show_settings(&state);
		}
		if !held {
			self.show_level(&current);
		}
	}

	/// Rewrites the status box when its text changes, keeping the caret on the same line.
	fn set_status(&self, text: &str) {
		if self.status.get_value() == text {
			return;
		}
		let (column, line) = self.status.position_to_xy(self.status.get_insertion_point()).unwrap_or((0, 0));
		self.status.change_value(text);
		let last_line = i64::from(self.status.get_number_of_lines().max(1)) - 1;
		let line = line.min(last_line);
		let column = column.min(i64::from(self.status.get_line_length(line)));
		let position = self.status.xy_to_position(column, line);
		if position >= 0 {
			self.status.set_insertion_point(position);
		}
	}

	fn show_settings(&self, state: &ReceiverState) {
		select(self.mode, index_of(OutputMode::ALL, state.mode));
		select(self.gain, index_of(OutputGain::ALL, state.gain));
		select(self.led, index_of(LedBrightness::ALL, state.led));
	}

	/// The transmitters the noise reduction target names.
	fn targets(&self) -> Vec<Transmitter> {
		match self.nr_target.get_selection() {
			Some(1) => vec![Transmitter::Tx1],
			Some(2) => vec![Transmitter::Tx2],
			_ => Transmitter::ALL.to_vec(),
		}
	}

	/// Shows the noise reduction level for the chosen target: its value, "Mixed" when "Both" is
	/// chosen and they differ, or disabled when no target transmitter is connected.
	fn show_level(&self, snapshot: &Snapshot) {
		let levels: Vec<NoiseReduction> = match snapshot {
			Snapshot::Receiver(state) => self
				.targets()
				.into_iter()
				.filter_map(|tx| match state.tx(tx) {
					TxStatus::Connected { noise_reduction, .. } => Some(noise_reduction),
					TxStatus::Disconnected => None,
				})
				.collect(),
			_ => Vec::new(),
		};
		self.nr_level.enable(!levels.is_empty());
		let Some(&first) = levels.first() else {
			return;
		};
		let mixed = levels.iter().any(|&level| level != first);
		if mixed != self.level_mixed.get() {
			// The list itself changes, so rebuild it.
			self.nr_level.clear();
			for label in labels(NoiseReduction::ALL) {
				self.nr_level.append(&label);
			}
			if mixed {
				self.nr_level.append(MIXED);
			}
			self.level_mixed.set(mixed);
		}
		select(self.nr_level, if mixed { NoiseReduction::ALL.len() } else { index_of(NoiseReduction::ALL, first) });
	}

	/// Keeps readings from moving the controls back while a change reaches the receiver.
	fn hold(&self) {
		self.hold_until.set(Some(Instant::now() + HOLD_AFTER_CHANGE));
	}

	/// Registers `chord` as the speak-status hotkey, replacing any before it. Returns false if
	/// Windows refused it, usually because another program holds it.
	fn register_hotkey(&self, chord: Option<&KeyChord>) -> bool {
		drop(self.hotkeys.borrow_mut().take());
		let Some(chord) = chord else {
			return true;
		};
		let latest = Latest::clone(&self.latest);
		let (running, failed) = GlobalHotkeys::register(vec![(chord.clone(), ())], move |()| {
			let text = summary(&read_latest(&latest));
			post_to_ui(move || speech::output(&text));
		});
		*self.hotkeys.borrow_mut() = running;
		failed.is_empty()
	}

	fn warn_unregistered(&self, chord: &KeyChord) {
		show_warning(
			&self.frame,
			format!(
				"{} couldn't be set up as the speak status hotkey. Another program may be using it. \
				 Choose a different one in Settings.",
				chord.to_display_string()
			),
			APP_NAME,
		);
	}

	fn open_settings(&self) {
		if self.settings_open.replace(true) {
			return;
		}
		// Unregistered while the dialog is open, so pressing the current hotkey there is captured
		// rather than spoken.
		drop(self.hotkeys.borrow_mut().take());
		let current = self.config.borrow().clone();
		let edited = settings::show(&self.frame, &current);
		let mut config = edited.unwrap_or_else(|| current.clone());
		if config.start_with_windows != current.start_with_windows
			&& let Err(error) = startup::set_enabled(config.start_with_windows)
		{
			show_error(&self.frame, format!("Couldn't change Start with Windows: {error}"), APP_NAME);
			config.start_with_windows = current.start_with_windows;
		}
		let wanted = config.hotkey();
		if !self.register_hotkey(wanted.as_ref()) {
			if let Some(chord) = &wanted {
				self.warn_unregistered(chord);
			}
			// Keep the hotkey that worked before.
			config.set_hotkey(current.hotkey().as_ref());
			self.register_hotkey(current.hotkey().as_ref());
		}
		if config != current {
			if let Err(error) = config.save() {
				show_error(&self.frame, format!("Couldn't save the settings: {error}"), APP_NAME);
			}
			*self.config.borrow_mut() = config;
		}
		self.settings_open.set(false);
	}

	fn about(&self) {
		AboutBoxBuilder::new(&self.frame)
			.name(APP_NAME)
			.version(env!("CARGO_PKG_VERSION"))
			.description("An accessible control panel for the RØDE Wireless Micro.")
			.show();
	}
}
