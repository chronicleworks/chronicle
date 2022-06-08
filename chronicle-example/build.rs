use std::process::Command;

use chronicle::codegen::ChronicleDomainDef;
use chronicle::{generate_chronicle_domain_schema, Builder, PrimitiveType};

fn main() {
    let s = r#"
    name: "chronicle"
    attributes:
      string:
        typ: "String"
      int:
        typ: "Int"
      bool:
        typ: "Bool"
    agents:
      pals:
        string:
          typ: "String"
        int:
          typ: "Int"
        bool:
          typ: "Bool"
    entities:
      octopi:
        string:
          typ: "String"
        int:
          typ: "Int"
        bool:
          typ: "Bool"
      the sea:
        string:
          typ: "String"
        int:
          typ: "Int"
        bool:
          typ: "Bool"
    activities:
      gardening:
        string:
          typ: "String"
        int:
          typ: "Int"
        bool:
          typ: "Bool"
      swim about:
        string:
          typ: "String"
        int:
          typ: "Int"
        bool:
          typ: "Bool"
     "#
    .to_string();

    let model = ChronicleDomainDef::from_input_string(&s).unwrap();

    generate_chronicle_domain_schema(model, "src/main.rs");

    Command::new("cargo")
        .args(["fmt", "--", "src/main.rs"])
        .output()
        .expect("formatting");
}
