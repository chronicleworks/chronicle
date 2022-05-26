use std::process::Command;

use chronicle::{generate_chronicle_domain_schema, Builder, PrimitiveType};

fn main() {
    let model = Builder::new("chronicle")
        .with_attribute_type("string", PrimitiveType::String)
        .unwrap()
        .with_attribute_type("int", PrimitiveType::Int)
        .unwrap()
        .with_attribute_type("bool", PrimitiveType::Bool)
        .unwrap()
        .with_entity("octopi", |b| {
            b.with_attribute("string")
                .unwrap()
                .with_attribute("int")
                .unwrap()
                .with_attribute("bool")
        })
        .unwrap()
        .with_entity("the sea", |b| {
            b.with_attribute("string")
                .unwrap()
                .with_attribute("int")
                .unwrap()
                .with_attribute("bool")
        })
        .unwrap()
        .with_activity("gardening", |b| {
            b.with_attribute("string")
                .unwrap()
                .with_attribute("int")
                .unwrap()
                .with_attribute("bool")
        })
        .unwrap()
        .with_activity("swim about", |b| {
            b.with_attribute("string")
                .unwrap()
                .with_attribute("int")
                .unwrap()
                .with_attribute("bool")
        })
        .unwrap()
        .with_agent("friends", |b| {
            b.with_attribute("string")
                .unwrap()
                .with_attribute("int")
                .unwrap()
                .with_attribute("bool")
        })
        .unwrap()
        .build();

    generate_chronicle_domain_schema(model, "src/main.rs");

    Command::new("cargo")
        .args(["fmt", "--", "src/main.rs"])
        .output()
        .expect("formatting");
}
