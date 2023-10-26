use std::process::Command;

use chronicle::{
	codegen::{linter, ChronicleDomainDef},
	generate_chronicle_domain_schema,
};

fn main() {
	println!("cargo:rerun-if-changed=domain.yaml");
	linter::check_files(vec!["domain.yaml"]);
	let model = ChronicleDomainDef::from_file("domain.yaml").unwrap();

	generate_chronicle_domain_schema(model, "src/main.rs");

	Command::new("cargo")
		.args(["fmt", "--", "src/main.rs"])
		.output()
		.expect("formatting");

	//Create a .VERSION file containing 'local' if it does not exist

	let version_file = std::path::Path::new("../../.VERSION");
	if !version_file.exists() {
		std::fs::write(version_file, "local").expect("Unable to write file");
	}
}
