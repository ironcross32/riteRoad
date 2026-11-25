//! Installing riteRoad to this PC, and uninstalling it.
//!
//! There's no installer. Every copy is portable, and "installing" copies the running one to
//! `%LOCALAPPDATA%\Programs\riteRoad`, then registers that copy with Windows: a Start menu
//! shortcut, an Add/Remove Programs entry, and optionally a desktop shortcut and Start with
//! Windows. The installed copy keeps its settings beside it like any other, and updates the same
//! way, from the release zip. All of it is per user, so nothing asks for administrator rights.

use std::{
	env, fs, io,
	os::windows::process::CommandExt,
	path::{Path, PathBuf},
	process::{self, Command},
};

use windows::{
	Win32::{
		System::Com::{
			CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
			CoUninitialize, IPersistFile,
		},
		UI::Shell::{
			FOLDERID_Desktop, FOLDERID_Programs, IShellLinkW, KF_FLAG_DEFAULT, SHGetKnownFolderPath, ShellLink,
		},
	},
	core::{GUID, HSTRING, Interface},
};

use crate::{
	app::APP_NAME,
	config::{self, PANIC_LOG},
	registry, startup,
};

pub const EXE_NAME: &str = "riteroad.exe";
/// Add/Remove Programs runs the installed copy with this to uninstall it.
pub const UNINSTALL_ARG: &str = "--uninstall";
const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\riteRoad";
const SHORTCUT_NAME: &str = "riteRoad.lnk";
const PUBLISHER: &str = "ironcross32";
const HOMEPAGE: &str = "https://github.com/ironcross32/riteRoad";
/// The release zip's other files, copied along with the executable when they're there.
const COMPANIONS: [&str; 2] = ["README.html", "LICENSE.txt"];
/// Keeps the helper scripts' PowerShell window from flashing up.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// What the install dialog asked for.
#[derive(Debug, Clone, Copy)]
pub struct Options {
	pub desktop_shortcut: bool,
	pub start_with_windows: bool,
}

/// `%LOCALAPPDATA%\Programs\riteRoad`.
pub fn install_dir() -> Option<PathBuf> {
	env::var_os("LOCALAPPDATA").map(|dir| PathBuf::from(dir).join("Programs").join(APP_NAME))
}

fn installed_exe() -> Option<PathBuf> {
	install_dir().map(|dir| dir.join(EXE_NAME))
}

/// Whether the running copy is the installed one.
pub fn is_installed_copy() -> bool {
	match (config::app_dir(), install_dir()) {
		(Some(here), Some(installed)) => startup::same_file(&here, &installed),
		_ => false,
	}
}

/// Whether there's an installed copy, whichever copy is running.
pub fn is_installed() -> bool {
	installed_exe().is_some_and(|exe| exe.is_file())
}

/// Whether Windows starts the installed copy at sign-in.
pub fn installed_copy_starts_with_windows() -> bool {
	installed_exe().is_some_and(|exe| startup::starts(&exe))
}

pub fn desktop_shortcut_exists() -> bool {
	known_folder(&FOLDERID_Desktop).is_some_and(|dir| dir.join(SHORTCUT_NAME).is_file())
}

/// Copies this copy and its settings to the install folder, replacing an installed copy that's
/// already there, and registers it with Windows.
pub fn install(options: Options) -> io::Result<()> {
	let source = config::app_dir().ok_or_else(|| io::Error::other("couldn't find riteRoad's folder"))?;
	let target = install_dir().ok_or_else(|| io::Error::other("LOCALAPPDATA is not set"))?;
	fs::create_dir_all(&target)?;
	let exe = target.join(EXE_NAME);
	fs::copy(env::current_exe()?, &exe)?;
	// The installed copy gets this copy's settings, or the defaults if it has none.
	let settings = source.join(config::FILE_NAME);
	if settings.is_file() {
		fs::copy(settings, target.join(config::FILE_NAME))?;
	} else {
		remove_if_there(&target.join(config::FILE_NAME))?;
	}
	for name in COMPANIONS {
		let file = source.join(name);
		if file.is_file() {
			fs::copy(file, target.join(name))?;
		}
	}
	register_uninstaller(&target, &exe)?;
	let programs = known_folder(&FOLDERID_Programs).ok_or_else(|| io::Error::other("couldn't find the Start menu"))?;
	create_shortcut(&programs.join(SHORTCUT_NAME), &exe, &target)?;
	if let Some(desktop) = known_folder(&FOLDERID_Desktop) {
		let shortcut = desktop.join(SHORTCUT_NAME);
		if options.desktop_shortcut {
			create_shortcut(&shortcut, &exe, &target)?;
		} else {
			remove_if_there(&shortcut)?;
		}
	}
	if options.start_with_windows { startup::enable_for(&exe) } else { startup::disable() }
}

/// Starts the installed copy once this process has exited, since riteRoad runs one copy at a time.
/// The caller then exits.
pub fn start_installed_copy_after_exit() -> io::Result<()> {
	let exe = installed_exe().ok_or_else(|| io::Error::other("LOCALAPPDATA is not set"))?;
	// The marker is ship-shape's, which lets the new window come to the front, as it does after an
	// update. Windows otherwise opens it behind whatever has the focus.
	run_after_exit(&format!(
		"$env:SHIP_SHAPE_RELAUNCHED='1'; Start-Process -FilePath {} -WorkingDirectory {}",
		ps_quote(&exe),
		ps_quote(exe.parent().unwrap_or(&exe))
	))
}

/// Unregisters the installed copy and removes it once this process has exited, keeping its
/// settings unless `delete_settings`. Works from any copy; only the installed one needs to exit
/// afterwards.
pub fn uninstall(delete_settings: bool) -> io::Result<()> {
	let target = install_dir().ok_or_else(|| io::Error::other("LOCALAPPDATA is not set"))?;
	if installed_copy_starts_with_windows() {
		startup::disable()?;
	}
	for folder in [&FOLDERID_Programs, &FOLDERID_Desktop] {
		if let Some(dir) = known_folder(folder) {
			remove_if_there(&dir.join(SHORTCUT_NAME))?;
		}
	}
	registry::delete_key(UNINSTALL_KEY)?;
	let mut files = vec![EXE_NAME, PANIC_LOG];
	files.extend(COMPANIONS);
	if delete_settings {
		files.push(config::FILE_NAME);
	}
	let paths: Vec<String> = files.iter().map(|name| ps_quote(&target.join(name))).collect();
	// The executable can't be deleted until its process has gone, and Windows can hold on to it a
	// moment longer, so keep trying for a few seconds. The folder goes too once it's empty.
	run_after_exit(&format!(
		"$files = @({}); foreach ($try in 1..20) {{ $files | Where-Object {{ Test-Path -LiteralPath $_ }} | ForEach-Object \
		 {{ Remove-Item -LiteralPath $_ -Force -ErrorAction SilentlyContinue }}; if (-not ($files | Where-Object {{ \
		 Test-Path -LiteralPath $_ }})) {{ break }}; Start-Sleep -Milliseconds 250 }}; $dir = {}; if (-not \
		 (Get-ChildItem -LiteralPath $dir -Force -ErrorAction SilentlyContinue)) {{ Remove-Item -LiteralPath $dir \
		 -Force -ErrorAction SilentlyContinue }}",
		paths.join(","),
		ps_quote(&target)
	))
}

/// Runs `script` in hidden PowerShell once this process has exited.
fn run_after_exit(script: &str) -> io::Result<()> {
	let script = format!("Wait-Process -Id {} -ErrorAction SilentlyContinue; {script}", process::id());
	Command::new("powershell.exe")
		.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &script])
		.creation_flags(CREATE_NO_WINDOW)
		.spawn()
		.map(drop)
}

fn ps_quote(path: &Path) -> String {
	format!("'{}'", path.display().to_string().replace('\'', "''"))
}

fn remove_if_there(path: &Path) -> io::Result<()> {
	match fs::remove_file(path) {
		Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
		other => other,
	}
}

/// The Add/Remove Programs entry, which runs `exe --uninstall`.
fn register_uninstaller(dir: &Path, exe: &Path) -> io::Result<()> {
	let quoted = format!("\"{}\"", exe.display());
	let size_kb = fs::metadata(exe).map_or(0, |m| u32::try_from(m.len().div_ceil(1024)).unwrap_or(u32::MAX));
	for (name, value) in [
		("DisplayName", APP_NAME.to_string()),
		("DisplayVersion", env!("CARGO_PKG_VERSION").to_string()),
		("Publisher", PUBLISHER.to_string()),
		("URLInfoAbout", HOMEPAGE.to_string()),
		("InstallLocation", dir.display().to_string()),
		("DisplayIcon", exe.display().to_string()),
		("UninstallString", format!("{quoted} {UNINSTALL_ARG}")),
	] {
		registry::set_string(UNINSTALL_KEY, name, &value)?;
	}
	registry::set_dword(UNINSTALL_KEY, "NoModify", 1)?;
	registry::set_dword(UNINSTALL_KEY, "NoRepair", 1)?;
	registry::set_dword(UNINSTALL_KEY, "EstimatedSize", size_kb)
}

/// Keeps the Add/Remove Programs entry's version current after an update. Only the installed copy
/// has an entry to keep.
pub fn refresh_uninstaller() {
	if is_installed_copy()
		&& let (Some(dir), Some(exe)) = (install_dir(), installed_exe())
	{
		let _ = register_uninstaller(&dir, &exe);
	}
}

fn known_folder(id: &GUID) -> Option<PathBuf> {
	// SAFETY: `id` is a valid folder ID, and the string Windows returns is freed exactly once, after
	// it's been copied.
	unsafe {
		let path = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).ok()?;
		let text = path.to_string().ok();
		CoTaskMemFree(Some(path.0.cast()));
		text.map(PathBuf::from)
	}
}

/// Writes a shortcut at `link` that starts `target` in `dir`.
fn create_shortcut(link: &Path, target: &Path, dir: &Path) -> io::Result<()> {
	// SAFETY: COM is initialised for this thread for the length of the calls, and every string
	// passed lives until the call it's passed to returns.
	unsafe {
		// wxWidgets has usually initialised COM on this thread already; this only adds a reference.
		let initialised = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
		let result = (|| -> windows::core::Result<()> {
			let shortcut: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
			shortcut.SetPath(&HSTRING::from(target.as_os_str()))?;
			shortcut.SetWorkingDirectory(&HSTRING::from(dir.as_os_str()))?;
			shortcut.SetDescription(&HSTRING::from("An accessible control panel for the RØDE Wireless Micro"))?;
			shortcut.cast::<IPersistFile>()?.Save(&HSTRING::from(link.as_os_str()), true)
		})();
		if initialised {
			CoUninitialize();
		}
		result.map_err(|error| io::Error::other(format!("couldn't create {}: {error}", link.display())))
	}
}
