use std::process::Command;

use chronicle::codegen::ChronicleDomainDef;
use chronicle::generate_chronicle_domain_schema;

fn main() {
    let s = r#"
name: "chronicle"
attributes:
  Title:
    type: "String"
  Location:
    type: "String"
  PurchaseValue:
    type: "String"
  PurchaseValueCurrency:
    type: "String"
  Description:
    type: "String"
  Name:
    type: "String"
agents:
  Member:
    attributes:
      - Name
  Artist:
    attributes:
      - Name
entities:
  Artwork:
    attributes:
      - Title
  ArtworkDetails:
    attributes:
      - Title
      - Description
activities:
  Exhibited:
    attributes:
      - Location
  Created:
    attributes:
      - Title
  Sold:
    attributes:
      - PurchaseValue
      - PurchaseValueCurrency
  Transferred:
    attributes: []
roles:
  - Buyer
  - Seller
  - Broker
  - Editor
  - Creator
     "#
    .to_string();

    let model = ChronicleDomainDef::from_input_string(&s).unwrap();

    generate_chronicle_domain_schema(model, "src/main.rs");

    Command::new("cargo")
        .args(["fmt", "--", "src/main.rs"])
        .output()
        .expect("formatting");
}
