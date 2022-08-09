use std::process::Command;

use change_detection::ChangeDetection;
use chronicle::{codegen::ChronicleDomainDef, generate_chronicle_domain_schema};

fn main() {
    ChangeDetection::path("domain.yaml").generate();
    let model = ChronicleDomainDef::from_file("domain.yaml").unwrap();
    generate_chronicle_domain_schema(model, "src/main.rs");

    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    Command::new("cargo")
        .args(["fmt", &format!("{}cli/src/main.rs", dir)])
        .output()
        .expect("formatting");
}
