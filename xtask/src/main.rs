//! `cargo xtask release`: builds riteRoad and packages it for a GitHub release.
//! `cargo xtask keygen <folder>`: makes the minisign key pair releases are signed with.

use std::{
	env,
	error::Error,
	fmt::Write as _,
	fs,
	path::{Path, PathBuf},
	process::ExitCode,
};

use minisign::KeyPair;
use shipfitter::{
	host_arch_suffix,
	package::{Zip, cargo_build_release},
	sign::Minisign,
};

const EXE_NAME: &str = "riteroad.exe";

fn main() -> ExitCode {
	let args: Vec<String> = env::args().skip(1).collect();
	let result = match args.first().map(String::as_str) {
		Some("release") => release(),
		Some("keygen") => match args.get(1) {
			Some(folder) => keygen(Path::new(folder)),
			None => Err("Give a folder to write the keys to, outside the repository.".into()),
		},
		_ => {
			println!("Tasks:");
			println!("	release            Build riteRoad in release mode, then package and sign it as a zip");
			println!("	keygen <folder>    Make a new signing key pair in <folder>");
			Ok(())
		}
	};
	// Printed here rather than returned from main, which would print it with Debug, in quotes.
	if let Err(error) = result {
		eprintln!("Error: {error}");
		return ExitCode::FAILURE;
	}
	ExitCode::SUCCESS
}

/// Builds riteRoad, then packages it in `target/release` as `riteroad-<arch>.zip`, such as
/// `riteroad-x64.zip`, with the executable at the top of the zip so an update unzips straight over
/// the folder it runs from. The settings file is never in it. With the minisign key in
/// `MINISIGN_KEY` and its password in `MINISIGN_PASSWORD`, the zip gets a `.minisig` signature
/// beside it, which the updater checks before installing anything.
fn release() -> Result<(), Box<dyn Error>> {
	// The key is read before anything is built, so a broken secret fails at once.
	let minisign = Minisign::from_env()?;
	let zip = package()?;
	match minisign {
		Some(minisign) => println!("Signed {}", minisign.sign(&zip)?.display()),
		None => println!("MINISIGN_KEY isn't set, so nothing is signed, and the updater won't install this."),
	}
	Ok(())
}

/// Builds and zips riteRoad, returning the zip's path.
fn package() -> Result<PathBuf, Box<dyn Error>> {
	let root = project_root();
	cargo_build_release(&root, &["riteroad"])?;
	let target_dir = root.join("target/release");
	let exe = target_dir.join(EXE_NAME);
	// The build script converts the README beside the executable.
	let readme = target_dir.join("README.html");
	for built in [&exe, &readme] {
		if !built.exists() {
			return Err(format!("{} wasn't built", built.display()).into());
		}
	}
	let zip_path = target_dir.join(format!("riteroad-{}.zip", host_arch_suffix()));
	let mut zip = Zip::create(&zip_path)?;
	zip.file(&exe, EXE_NAME)?;
	zip.file(&readme, "README.html")?;
	let license = root.join("LICENSE");
	if license.exists() {
		zip.file(&license, "LICENSE.txt")?;
	}
	zip.finish()?;
	println!("Created zip: {}", zip_path.display());
	Ok(zip_path)
}

/// Writes a new key pair to `folder`: `riteroad.key`, the secret key encrypted with a random
/// password, `password.txt`, and `riteroad.pub`. Refuses to overwrite a key that's already there,
/// since releases signed with it couldn't be checked any more.
fn keygen(folder: &Path) -> Result<(), Box<dyn Error>> {
	let secret_path = folder.join("riteroad.key");
	if secret_path.exists() {
		return Err(format!(
			"{} already exists. Move it away first if you really mean to replace it.",
			secret_path.display()
		)
		.into());
	}
	let mut bytes = [0u8; 24];
	getrandom::fill(&mut bytes).map_err(|e| format!("couldn't get random bytes: {e}"))?;
	let password = bytes.iter().fold(String::new(), |mut text, byte| {
		let _ = write!(text, "{byte:02x}");
		text
	});
	let pair = KeyPair::generate_encrypted_keypair(Some(password.clone()))?;
	let public = pair.pk.to_box()?.into_string();
	fs::create_dir_all(folder)?;
	fs::write(&secret_path, pair.sk.to_box(Some("riteRoad release signing key"))?.into_string())?;
	fs::write(folder.join("password.txt"), &password)?;
	fs::write(folder.join("riteroad.pub"), &public)?;
	println!("Wrote the key pair to {}", folder.display());
	println!("Public key for src/update.rs: {}", public.lines().nth(1).unwrap_or_default());
	Ok(())
}

fn project_root() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("xtask lives one level below the project").to_path_buf()
}
