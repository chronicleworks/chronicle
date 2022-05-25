use std::process::Command;

use chronicle::{generate_chronicle_domain_schema, Builder};

fn main() {
    let model = Builder::new("chronicle").build();

    generate_chronicle_domain_schema(model, "src/main.rs");

    Command::new("cargo")
        .args(["fmt", "--", "src/main.rs"])
        .output()
        .expect("formatting");
}
