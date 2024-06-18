use std::{collections::HashSet, path::Path, process::exit};

use jsonschema::{error::ValidationErrorKind, JSONSchema};

use common::domain::{DomainFileInput, ResourceDef};

fn bad_filename(filename: &str) {
	println!("JSON or YAML filename extension required for {filename}");
	exit(2);
}

fn build_json_validator(domain: &str) -> JSONSchema {
	let json = match serde_json::from_str(domain) {
		Ok(json) => json,
		Err(error) => {
			println!("failed to parse valid JSON from domain schema: {error}");
			exit(2);
		},
	};
	match JSONSchema::options().with_draft(jsonschema::Draft::Draft7).compile(&json) {
		Ok(json_schema) => json_schema,
		Err(error) => {
			println!("failed to interpret JSON as a domain schema: {error}");
			exit(2);
		},
	}
}

fn check_json_valid(json_validator: &JSONSchema, json_data: &str) {
	let json = match serde_json::from_str(json_data) {
		Ok(json) => json,
		Err(error) => {
			println!("failed to parse valid JSON: {error}");
			exit(2);
		},
	};
	let validation = json_validator.validate(&json);
	if let Err(errors) = validation {
		for error in errors {
			println!("path {} contains invalid data: {}", error.instance_path, error);
			if let ValidationErrorKind::Pattern { pattern } = error.kind {
				match pattern.as_str() {
					"^[A-Z][A-Z0-9_]*$" => {
						println!("hint: start with capital letter, use only CAPITALS_UNDERSCORES_NUM8ER5");
					},
					"[A-Z][A-Za-z0-9]*$" => {
						println!("hint: start with capital letter, use only LettersAndNum8er5");
					},
					_ => {},
				}
			}
		}
		exit(2);
	}
}

fn check_yaml_valid(json_validator: &JSONSchema, yaml_data: &str) {
	let json = match serde_yaml::from_str::<serde_json::Value>(yaml_data) {
		Ok(json) => json,
		Err(error) => {
			println!("failed to parse valid YAML: {error}");
			exit(2);
		},
	};
	let json_data = match serde_json::to_string(&json) {
		Ok(json_data) => json_data,
		Err(error) => {
			println!("failed to write valid JSON from YAML: {error}");
			exit(2);
		},
	};
	check_json_valid(json_validator, &json_data);
}

fn read_json_domain(data: &str) -> DomainFileInput {
	match serde_json::from_str(data) {
		Ok(domain) => domain,
		Err(error) => {
			println!("failed to interpret JSON as a domain: {error}");
			exit(2);
		},
	}
}

fn read_yaml_domain(data: &str) -> DomainFileInput {
	match serde_yaml::from_str(data) {
		Ok(domain) => domain,
		Err(error) => {
			println!("failed to interpret YAML as a domain: {error}");
			exit(2);
		},
	}
}

fn check_domain_attributes(
	element: &str,
	attributes: &HashSet<String>,
	named_resources: Vec<(&String, &ResourceDef)>,
) {
	let mut is_error = false;
	for (name, resource) in named_resources {
		for attribute in resource.attributes.iter() {
			if !(attributes.contains(&attribute.0)) {
				println!("{} named {} has unknown attribute {}", element, name, attribute.0);
				is_error = true;
			}
		}
	}
	if is_error {
		exit(2);
	}
}

fn check_domain(domain: DomainFileInput) {
	let attributes = domain.attributes.keys().map(std::clone::Clone::clone).collect();
	check_domain_attributes("agent", &attributes, domain.agents.iter().collect());
	check_domain_attributes("entity", &attributes, domain.entities.iter().collect());
	check_domain_attributes("activity", &attributes, domain.activities.iter().collect());
}

pub fn check_files(filenames: Vec<&str>) {
	let json_validator = build_json_validator(include_str!("../../schema/domain.json"));
	for filename in filenames {
		let filepath = Path::new(filename);
		let data = match std::fs::read_to_string(filepath) {
			Ok(data) => data,
			Err(error) => {
				println!("failed to read {filename}: {error}");
				exit(2);
			},
		};
		match filepath.extension() {
			Some(extension) => {
				match extension.to_ascii_lowercase().to_str() {
					Some("json") | Some("jsn") => {
						check_json_valid(&json_validator, data.as_str());
						check_domain(read_json_domain(&data));
					},
					Some("yaml") | Some("yml") => {
						check_yaml_valid(&json_validator, data.as_str());
						check_domain(read_yaml_domain(&data));
					},
					_ => {
						bad_filename(filename);
					},
				};
			},
			None => {
				bad_filename(filename);
			},
		};
	}
}
