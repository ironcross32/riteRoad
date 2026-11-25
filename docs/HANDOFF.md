# riteRoad handoff

Updated 2026-11-25. Pick up here next session.

## Where things stand

- The receiver's HID protocol is decoded and documented in `docs/PROTOCOL.md`.
- The control library (`src/protocol.rs`, `src/device.rs`) and the test tool
  `riteroad-cli` work against the real receiver.
- The wxDragon app is built to the agreed design in `docs/DESIGN.md`. The user
  has tested every microphone control, transmitter disconnects, and Exit from
  the tray.
- GitHub: `ironcross32/riteRoad`, branch `master`, one commit pushed by the user.
- **Self-install and ship-shape updates are written but untested by the
  user, and uncommitted** (2026-11-25):
  - `src/config.rs`: `settings.toml` beside the exe; `start_with_windows` is
    gone, `check_for_updates` added (on by default).
  - `src/startup.rs`: Start with Windows read from the `Run` value, compared
    with the running exe. `src/registry.rs` holds the registry calls.
  - `src/install.rs`, `src/install_ui.rs`: install, uninstall, shortcuts, the
    Add/Remove Programs entry, and their dialogs.
  - `src/update.rs`: ship-shape, portable, `-x64`/`-arm64`.
  - `src/settings.rs`, `src/app.rs`, `src/main.rs`: the Startup button, the
    Updates group, `--uninstall`, and routing every exit through `exiting`.
  - `xtask/`: `cargo xtask release` and `cargo xtask keygen <folder>`.
  - `build.rs` converts `README.md` to `README.html` beside the exe.
  - `.github/workflows/ci.yml` and `release.yml`, adapted from headroom.
  - `README.md` (a first draft the user will rewrite) and `CHANGELOG.md`.
- Signing key: made with `cargo xtask keygen` in `%USERPROFILE%\riteRoad-signing`
  (`riteroad.key`, `password.txt`, `riteroad.pub`). The public key is in
  `src/update.rs`. The user must back that folder up somewhere safe.

## How the pieces fit

- An update, an install handing over, and an uninstall all exit through
  `App::exiting` (see the module doc in `src/app.rs`). ship-shape's exit
  handler sets `App::update_exit`, an `Arc<AtomicBool>`, because the handler
  must be `Send` and `App` isn't. The tick sees it, and if Settings is open,
  ends the dialog with OK first, so its changes are saved.
- riteRoad is single instance. A second launch passes its arguments to the
  running copy through doorbell, so `riteroad.exe --uninstall` from
  Add/Remove Programs reaches the running copy, which shows the uninstall
  dialog. With no copy running, `app::uninstall_alone` shows only the dialog.
- The installed copy can't delete itself, and it can't start while the
  portable copy runs, so both handovers use a hidden PowerShell helper that
  waits for this process to exit (`install::run_after_exit`). The install
  handover sets ship-shape's `SHIP_SHAPE_RELAUNCHED` marker, so
  `bring_to_front_after_update` brings the installed copy's window forward.
- The update check's dialogs are parented to the main frame, not Settings, so
  closing Settings mid-check can't leave them without a parent.
- The automatic check skips silently when the folder isn't writable. The
  button shows a plain error box instead.

## To test (with NVDA)

1. From `target\debug` (portable): Settings shows Install to this PC, and
   Start with Windows is unticked. The old `Run` value points at
   `target\debug\riteroad.exe`, so it may show ticked. Untick it once.
2. Tick Start with Windows: the install dialog opens with Start with Windows
   ticked. Cancel unticks the box and writes nothing.
3. Install: the portable copy closes, and the installed copy opens in front,
   with the same settings. Check the Start menu shortcut, Installed apps, and
   the `Run` value.
4. In the installed copy: the button reads Uninstall from this PC.
5. Uninstall from Installed apps, both with riteRoad running and without it.
   The folder should be empty, or hold only `settings.toml`.
6. Check for updates with no release yet: it should report an error, or
   that it couldn't find an update.
7. The hotkey field in Settings is reachable with Tab, and Enter still means
   OK (from the last session; never confirmed).
8. File > Exit from the menu (never tested; same code path as the other
   exits).

## Release

1. Put the key in GitHub secrets: `MINISIGN_KEY` gets the contents of
   `riteroad.key`, and `MINISIGN_PASSWORD` gets `password.txt`.
2. The user rewrites `README.md`, then commits and pushes.
3. To release: bump `version` in `Cargo.toml`, add a `## <version>` section
   to `CHANGELOG.md`, tag `v<version>`, and push the tag. The first is
   `v0.1.0`, which is already in both files.

## Notes

- Don't send keystrokes to the desktop for testing without asking the user to
  step away first. They use the PC while Claude works.
- The old settings in `%LOCALAPPDATA%\riteRoad` aren't migrated. The user
  will copy or delete them by hand.
- Add a `LICENSE` file and `cargo xtask release` puts it in the zip as
  `LICENSE.txt`. There's none yet; the user hasn't chosen one.
