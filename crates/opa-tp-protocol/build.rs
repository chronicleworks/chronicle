use std::{env, fs, io::Result, path::PathBuf};

fn main() -> Result<()> {
	let protos = glob::glob("./src/protos/*.proto")
		.unwrap()
		.map(|x| x.unwrap())
		.collect::<Vec<_>>();
	prost_build::compile_protos(&protos, &["./src/protos"])?;

	let out_str = env::var("OUT_DIR").unwrap();
	let out_path = PathBuf::from(&out_str);
	let mut out_path = out_path.ancestors().nth(3).unwrap().to_owned();
	out_path.push("assets");

	if !out_path.exists() {
		fs::create_dir(&out_path).expect("Could not create assets dir");
	}

	Ok(())
}
