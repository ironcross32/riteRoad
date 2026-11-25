use std::path::Path;

use shipfitter::{
	build::target_profile_dir,
	docs::{Page, convert},
	windows::{VersionInfo, embed_manifest},
};

fn main() {
	println!("cargo:rerun-if-changed=build.rs");
	println!("cargo:rerun-if-changed=Cargo.toml");
	println!("cargo:rerun-if-changed=README.md");
	// Common Controls v6 and DPI awareness. Without it wxWidgets warns at startup.
	embed_manifest("riteRoad").expect("Could not embed the Windows application manifest");
	VersionInfo { product_name: "riteRoad", original_filename: "riteroad.exe", ..VersionInfo::default() }
		.embed()
		.expect("Could not embed the Windows version information");
	// The README as a page beside the executable, which `cargo xtask release` puts in the zip.
	if let Some(target_dir) = target_profile_dir() {
		convert(
			Path::new("README.md"),
			&target_dir.join("README.html"),
			&Page { title: "riteRoad", ..Page::default() },
		)
		.expect("Could not convert README.md");
	}
}
