use std::process::Command;

use chronicle::{generate_chronicle_domain_schema, Builder};

fn main() {
    let model = Builder::new("chronicle").build();

    generate_chronicle_domain_schema(model, "src/main.rs");

    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    Command::new("cargo")
        .args(["fmt", &format!("{}cli/src/main.rs", dir)])
        .output()
        .expect("formatting");
}
