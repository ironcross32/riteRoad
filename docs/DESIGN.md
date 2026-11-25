# riteRoad: app design

An accessible replacement for RØDE Central for the RØDE Wireless Micro
receiver, built with wxDragon. Agreed with the user on 2026-11-24.

## Main window: "riteRoad"

Tab order:

1. **Status** — read-only multi-line text box, one item per line:
   ```
   Receiver: connected
   Output mode: Split
   Output gain: High
   LED brightness: Dim
   Transmitter 1: connected, battery 86%, noise reduction Off
   Transmitter 2: not connected
   ```
   Without a receiver the box contains only:
   "The RØDE Wireless Micro receiver is not plugged in. Plug it into a USB
   port to use riteRoad."
   The box is rewritten only when its text changes, and the caret is kept on
   the same line number.
2. **Receiver** group
   - Output mode: Split, Merged
   - Output gain: Low, Medium, High
   - LED brightness: Dim, Bright
3. **Noise reduction** group
   - Transmitter: Both, Transmitter 1, Transmitter 2
   - Level: Off, Low, High. While "Both" is selected and the transmitters
     differ, a temporary "Mixed" item is shown and selected.
   - "Both" writes one transmitter at a time and skips a disconnected one.
   - A single disconnected target disables Level.
4. **Exit** button, mnemonic Alt+X. No other control on the window has a
   mnemonic.

All microphone controls are disabled while the receiver is absent and are
re-enabled (with values refreshed) when it returns. Changes are sent to the
receiver immediately; there is no Apply button.

## Menus

- &File: &Settings (Ctrl+comma), E&xit
- &Help: &About riteRoad

## Tray icon

- Click: show the main window.
- Menu: Show riteRoad, Settings, Exit.
- Closing the main window (X / Alt+F4) hides it to the tray.
- Every Exit (menu, tray, button) goes through one shutdown path that stops
  the poll timer, unregisters the hotkey, removes the tray icon and destroys
  windows outside any event callback. The process must exit with code 0.

## Settings dialog (OK / Cancel, no mnemonics)

Changes take effect on OK; Cancel discards them.

1. **Hotkey** group: speak-status hotkey, captured by pressing it.
   Default Ctrl+Shift+F9. Delete or Backspace clears it ("None"). Tab and
   Shift+Tab are never captured. Must include Ctrl, Alt or Windows. If Windows
   refuses the combination, say so and keep the previous one.
2. **Automatic announcements** group (all on by default):
   - Receiver connected or disconnected
   - Transmitter connected or disconnected
   - Low battery warnings
3. **Startup** group:
   - Start with Windows. Not stored in the settings: the box shows whether
     the `Run` value points at the running exe. When started by Windows the
     app goes straight to the tray.
   - A button: "Install to this PC" on any copy that isn't the installed one,
     "Uninstall from this PC" on the installed copy. See "Install, uninstall
     and updates" below.
4. **Updates** group:
   - Check for updates at startup (on by default).
   - Check for updates (button). If the user accepts an update, the dialog's
     pending changes are saved first, as if OK had been pressed.

Stored in `settings.toml` beside `riteroad.exe`, so the settings travel with
the folder (flash drive, Dropbox). Nothing in it is per machine. There is no
migration from the old `%LOCALAPPDATA%\riteRoad` file.

## Install, uninstall and updates

Agreed with the user on 2026-11-25. The model is Sensor Readout's
(<https://github.com/OnjLouis/accessible-sensor-readout>): one portable zip
that can install itself. There is no Inno Setup installer.

- **Installed copy**: the one running from
  `%LOCALAPPDATA%\Programs\riteRoad`. Every other copy is portable.
- **Install dialog**: says where it will install; "Create desktop shortcut"
  (off by default); "Start with Windows" (ticked when reached by ticking Start
  with Windows, otherwise off); Install and Cancel.
- **Installing** copies only riteRoad's own files (`riteroad.exe`,
  `settings.toml`, `README.html`, license), then creates a Start menu
  shortcut and an Add/Remove Programs entry under HKCU whose uninstall command
  is `riteroad.exe --uninstall`. It adds a desktop shortcut and the `Run`
  value if those were ticked.
- **Already installed**: a portable copy still shows "Install to this PC".
  Pressing it first asks "riteRoad is already installed. Replace it with this
  copy and its settings?"
- **Handover**: the portable copy exits, then the installed copy opens with
  its window showing. The portable folder is left alone. riteRoad is single
  instance, so the installed copy must start only after the portable one has
  exited.
- **Start with Windows on a portable copy** offers to install first. Declining
  unticks the box and writes nothing to the registry, so the `Run` value only
  ever points at the installed copy. Moving a folder can't break startup.
- **Uninstall** removes the exe, shortcuts, `Run` value and Add/Remove
  Programs entry. It keeps `settings.toml` unless "Also delete settings" is
  ticked.

### Updates (ship-shape)

- Every copy is `InstallKind::Portable`. ship-shape unzips the release over
  the exe's folder, so the zip must have the exe at its root and must never
  contain `settings.toml` or `panic.log`.
- Assets: `riteroad-x64.zip` and `riteroad-arm64.zip`, each with a
  `.minisig`. Repository `ironcross32/riteRoad`, signed with a minisign key
  made for riteRoad alone.
- Updates go through ship-shape's exit handler, so they use the same
  shutdown as every other Exit and the window-close veto can't block them.
- The startup check is silent unless it finds an update, failures included.
  The button reports every result and every error.
- An update found at logon while in the tray shows its dialog straight away.
  The relaunched copy starts without `--tray`, so its window opens and comes
  to the front.
- If the folder isn't writable, show a plain error message with an OK button.

The panic log, `panic.log`, also sits beside `riteroad.exe`.

## Speech

Through the `prismer` crate, statically linked. All speech interrupts.

Hotkey summary:
- "Receiver not connected."
- otherwise "<Mode> mode." then, per transmitter, "Transmitter N, X percent."
  or "Transmitter N not connected."; both missing becomes
  "No transmitters connected."

Automatic announcements:
- "Receiver connected." / "Receiver disconnected."
- "Transmitter N connected." / "Transmitter N disconnected."
- "Transmitter N battery 20 percent." and "... 10 percent.", once each per
  discharge; reset when the level climbs back above the threshold.

## Other

- Single instance: starting riteRoad again shows the running copy's window.
- Help > About shows name, version and a short description.
- The Markdown README is converted to `README.html` during local and CI
  builds and shipped in the zip. The user will edit the README before the
  first push or release. An in-app link to it comes later.
- The first release is `v0.1.0`, built by CI from a pushed tag.
