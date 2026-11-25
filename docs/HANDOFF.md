# riteRoad handoff

Written 2026-11-24. Pick up here next session.

## Where things stand

- The receiver's HID protocol is decoded and documented in `docs/PROTOCOL.md`.
- The control library (`src/protocol.rs`, `src/device.rs`) and the test tool
  `riteroad-cli` work against the real receiver.
- The wxDragon app (`src/main.rs` and modules) is built to the agreed design
  in `docs/DESIGN.md`. The user has tested every microphone control,
  transmitter disconnects, and Exit from the tray, all good.
- Tests: 14 unit tests pass, and clippy is clean.
- **Nothing is committed yet.** The repo is on branch `master` with no
  commits, and has no GitHub remote yet.

## Loose ends

1. **The hotkey field in Settings should now be reachable with Tab, but the
   user hasn't confirmed it.** wxWidgets drops read-only single-line text
   fields from the Tab order, so the field is now multi-line. Enter in the
   field presses OK explicitly (`src/settings.rs`). Check with NVDA:
   - Tab and Shift+Tab reach the field.
   - Enter still means OK, and Escape still means Cancel.
   - Capturing a key and clearing with Delete still work.
2. **Start with Windows points at `target\debug\riteroad.exe`.** The user
   turned it on, and the registry Run value stores the debug build's path. An
   installer would fix this by giving a stable path.
3. **File > Exit hasn't been tested from the menu.** An earlier "works"
   result was wrong: the test keystrokes went to the user's own windows. The
   tray Exit (tested by the user) and the Exit button (tested) both exit with
   code 0. File > Exit uses the same code path.
4. **Panic log:** panics are written to `%LOCALAPPDATA%\riteRoad\panic.log`.
   None have occurred so far.
5. **Don't send keystrokes to the desktop for testing without asking the user
   to step away first.** They use the PC while Claude works.

## Next: self-install and ship-shape updates

Decided with the user on 2026-11-25. The agreed behaviour is in
`docs/DESIGN.md` (Settings dialog, and "Install, uninstall and updates").
This replaces the earlier Inno Setup plan, and settles loose end 2: the `Run`
value will only ever point at the installed copy.

Library: <https://github.com/accessware/ship-shape> (0.6.1 is in the cargo
cache). `C:\programming\headroom` is the working reference for `update.rs`,
`build.rs`, `xtask/`, `release.yml` and `CHANGELOG.md`. Skip its Inno Setup
template: riteRoad has no installer.

### App work

1. Config: move `settings.toml` and `panic.log` beside the exe, and drop
   `start_with_windows` from `Config`. `startup.rs` should report whether the
   `Run` value points at the running exe.
2. Install, uninstall, `--uninstall`, and the install dialog, as in
   `DESIGN.md`. riteRoad is single instance (`ring` in `src/main.rs`), so the
   installed copy must start only after the portable copy has exited. Use a
   detached helper that waits on the PID, as ship-shape's scripts do.
3. Settings: the Install/Uninstall button in Startup, and a new Updates group
   (a "Check for updates at startup" checkbox, on by default, then a "Check
   for updates" button).
4. `src/update.rs`: `UpdaterConfig::new("ironcross32/riteRoad", "riteroad",
   "riteRoad", PUBLIC_KEY, env!("CARGO_PKG_VERSION"))`, with
   `InstallKind::Portable` and an asset suffix of `-x64` or `-arm64` chosen
   by `cfg!(target_arch)`. Use
   `ui::run_update_check_with_exit_handler` with a handler that sets
   `App::exiting` (see the module doc in `src/app.rs`). Check that the folder
   is writable first, and show a plain error box if it isn't.
5. Call `ui::bring_to_front_after_update(&frame)` after the window is first
   shown. A relaunch after an update has no `--tray`, so the window shows.

### Release work

6. Create `ironcross32/riteRoad` on GitHub and generate a new minisign key
   pair. The public key goes in `update.rs`; the secret key and its password
   go into GitHub secrets.
7. CI builds x64 and ARM64 and publishes `riteroad-x64.zip` and
   `riteroad-arm64.zip` (exe at the zip root; never `settings.toml` or
   `panic.log`), each with a `.minisig`. It's triggered by `v*` tags.
8. Generate `README.html` from the README in local and CI builds. The user
   will edit the README before the first push.
9. The first release is `v0.1.0`. Routine: bump `Cargo.toml`, add a
   `## <version>` section to `CHANGELOG.md`, tag, and push the tag.
