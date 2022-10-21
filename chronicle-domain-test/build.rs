use std::process::Command;

use chronicle::{codegen::ChronicleDomainDef, generate_chronicle_domain_schema};

fn main() {
    let s = r#"
    name: "chronicle"
    attributes:
      NYU:
        type: "String"
      UCL:
        type: "Int"
      LSE:
        type: "Bool"
    agents:
      DOE:
        attributes:
          - NYU
          - UCL
          - LSE
    entities:
      NEH:
        attributes:
          - NYU
          - UCL
          - LSE
      NIH:
        attributes:
          - NYU
          - UCL
          - LSE
    activities:
      RND:
        attributes:
          - NYU
          - UCL
          - LSE
      RNR:
        attributes:
          - NYU
          - UCL
          - LSE
    roles:
        - VIP
        - SMH
     "#
    .to_string();

    let model = ChronicleDomainDef::from_input_string(&s).unwrap();

    generate_chronicle_domain_schema(model, "src/main.rs");

    Command::new("cargo")
        .args(["fmt", "--", "src/main.rs"])
        .output()
        .expect("formatting");
}
