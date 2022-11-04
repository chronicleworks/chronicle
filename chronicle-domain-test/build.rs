use std::process::Command;

use chronicle::{codegen::ChronicleDomainDef, generate_chronicle_domain_schema};

fn main() {
    let s = r#"
    name: "airworthiness"
    attributes:
      CertId:
        type: "String"
      BatchId:
        type: "String"
      PartId:
        type: "String"
      Location:
        type: "String"
      Manifest:
        type: JSON
    agents:
      Contractor:
        attributes:
          - Location
      NCB:
        attributes:
          - Manifest
    entities:
      Certificate:
        attributes:
          - CertId
      Item:
        attributes:
          - PartId
      NCBRecord:
        attributes: []
    activities:
      ItemCertified:
        attributes:
          - CertId
      ItemCodified:
        attributes: []
      ItemManufactured:
        attributes:
          - BatchId
    roles:
      - CERTIFIER
      - CODIFIER
      - MANUFACTURER
      - SUBMITTER
     "#
    .to_string();

    let model = ChronicleDomainDef::from_input_string(&s).unwrap();

    generate_chronicle_domain_schema(model, "src/main.rs");

    Command::new("cargo")
        .args(["fmt", "--", "src/main.rs"])
        .output()
        .expect("formatting");
}
