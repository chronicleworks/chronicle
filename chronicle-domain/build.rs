use std::process::Command;

use chronicle::{codegen::ChronicleDomainDef, generate_chronicle_domain_schema};

fn main() {
    let model = ChronicleDomainDef::from_file("domain.yaml").unwrap();

    generate_chronicle_domain_schema(model, "src/main.rs");

    Command::new("cargo")
        .args(["fmt", "--", "src/main.rs"])
        .output()
        .expect("formatting");
}
