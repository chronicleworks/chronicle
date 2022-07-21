use std::process::Command;

use chronicle::codegen::ChronicleDomainDef;
use chronicle::generate_chronicle_domain_schema;

fn main() {
    let s = r#"
    name: "chronicle"
    attributes:
      String:
        type: "String"
      Int:
        type: "Int"
      Bool:
        type: "Bool"
    agents:
      friend:
        attributes:
          - String
          - Int
          - Bool
    entities:
      octopi:
        attributes:
          - String
          - Int
          - Bool
      the sea:
        attributes:
          - String
          - Int
          - Bool
    activities:
      gardening:
        attributes:
          - String
          - Int
          - Bool
      swim about:
        attributes:
          - String
          - Int
          - Bool
    roles:
      - delegate
      - responsible
     "#
    .to_string();

    let model = ChronicleDomainDef::from_input_string(&s).unwrap();

    generate_chronicle_domain_schema(model, "src/main.rs");

    Command::new("cargo")
        .args(["fmt", "--", "src/main.rs"])
        .output()
        .expect("formatting");
}
