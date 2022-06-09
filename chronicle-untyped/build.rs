use std::process::Command;

use chronicle::{codegen::ChronicleDomainDef, generate_chronicle_domain_schema};

fn main() {
    let s = r#"
    name: "untyped"
    attributes: {}
    agents: {}
    entities: {}
    activities: {}
    "#
    .to_string();

    let model = ChronicleDomainDef::from_input_string(&s).unwrap();

    generate_chronicle_domain_schema(model, "src/main.rs");

    Command::new("cargo")
        .args(["fmt", "--", "src/main.rs"])
        .output()
        .expect("formatting");
}
