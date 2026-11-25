use shipfitter::windows::{VersionInfo, embed_manifest};

fn main() {
	println!("cargo:rerun-if-changed=build.rs");
	println!("cargo:rerun-if-changed=Cargo.toml");
	// Common Controls v6 and DPI awareness. Without it wxWidgets warns at startup.
	embed_manifest("riteRoad").expect("Could not embed the Windows application manifest");
	VersionInfo { product_name: "riteRoad", original_filename: "riteroad.exe", ..VersionInfo::default() }
		.embed()
		.expect("Could not embed the Windows version information");
}
