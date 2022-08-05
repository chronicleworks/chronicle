use std::process::Command;

use chronicle::{codegen::ChronicleDomainDef, generate_chronicle_domain_schema};

fn main() {
    let s = r#"
name: 'evidence'
attributes:
  Content:
    type: String
  CmsId:
    type: String
  Title:
    type: String
  SearchParameters:
    type: String
  Reference:
    type: String
  Version:
    type: Int
entities:
  Question:
    attributes:
      - CmsId
      - Content
  Evidence:
    attributes:
      - SearchParameters
      - Reference
  Guidance:
    attributes:
      - Title
      - Version
  PublishedGuidance:
    attributes: []
activities:
  QuestionAsked:
    attributes:
      - Content
  Researched:
    attributes:
      - SearchParameters
  Published:
    attributes:
      - Version
  Revised:
    attributes:
      - CmsId
      - Version
agents:
  Person:
    attributes:
      - CmsId
  Organization:
    attributes:
      - Title
roles:
  - Stakeholder
  - Author
  - Researcher
  - Editor
     "#
    .to_string();

    let model = ChronicleDomainDef::from_input_string(&s).unwrap();

    generate_chronicle_domain_schema(model, "src/main.rs");

    Command::new("cargo")
        .args(["fmt", "--", "src/main.rs"])
        .output()
        .expect("formatting");
}
