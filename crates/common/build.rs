use std::{env, fs, io::Result, path::PathBuf};

include!("./src/context.rs");

fn main() -> Result<()> {
	let out_str = env::var("OUT_DIR").unwrap();
	let out_path = PathBuf::from(&out_str);
	let mut out_path = out_path.ancestors().nth(3).unwrap().to_owned();
	out_path.push("assets");

	if !out_path.exists() {
		fs::create_dir(&out_path).expect("Could not create assets dir");
	}

	let context = &*PROV;

	std::fs::write(
		std::path::Path::new(&format!("{}/context.json", out_path.as_os_str().to_string_lossy(),)),
		serde_json::to_string_pretty(context)?,
	)?;

	Ok(())
}
