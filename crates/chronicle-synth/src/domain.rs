use std::{collections::BTreeMap, path::Path};

use serde::Deserialize;
use serde_json::json;

use common::domain::PrimitiveType;

use crate::{
    collection::{Collection, DomainCollection, Generator, Operation},
    error::ChronicleSynthError,
};

#[derive(Debug)]
pub struct TypesAttributesRoles {
    pub name: String,
    entities: BTreeMap<ParsedDomainType, BTreeMap<AttributeType, SynthType>>,
    agents: BTreeMap<ParsedDomainType, BTreeMap<AttributeType, SynthType>>,
    activities: BTreeMap<ParsedDomainType, BTreeMap<AttributeType, SynthType>>,
    roles: Vec<Role>,
}

impl TypesAttributesRoles {
    /// Creates a new `TypesAttributesRoles` instance from a YAML file at the specified path.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the YAML file.
    ///
    /// # Returns
    ///
    /// A `Result` containing the `TypesAttributesRoles` instance, or an error if the operation
    /// fails.
    pub fn from_file(path: &Path) -> Result<Self, ChronicleSynthError> {
        #[derive(Debug, Deserialize)]
        struct ChronicleDomain {
            #[serde(skip)]
            _roles_doc: Option<String>,
            roles: Vec<Role>,
            name: String,
            attributes: BTreeMap<AttributeType, ChroniclePrimitive>,
            entities: BTreeMap<ParsedDomainType, Attributes>,
            agents: BTreeMap<ParsedDomainType, Attributes>,
            activities: BTreeMap<ParsedDomainType, Attributes>,
        }

        impl ChronicleDomain {
            fn from_path(path: &Path) -> Result<Self, ChronicleSynthError> {
                let yaml: String = std::fs::read_to_string(path)?;
                let domain: ChronicleDomain = serde_yaml::from_str(&yaml)?;
                Ok(domain)
            }
        }

        impl From<ChronicleDomain> for TypesAttributesRoles {
            fn from(value: ChronicleDomain) -> Self {
                let mut attribute_types = BTreeMap::new();
                attribute_types.extend(value.attributes);

                let entities = value
                    .entities
                    .into_iter()
                    .map(|(entity_type, attributes)| {
                        (entity_type, attributes.link_attribute_types(&attribute_types))
                    })
                    .collect();
                let agents = value
                    .agents
                    .into_iter()
                    .map(|(agent_type, attributes)| {
                        (agent_type, attributes.link_attribute_types(&attribute_types))
                    })
                    .collect();
                let activities = value
                    .activities
                    .into_iter()
                    .map(|(activity_type, attributes)| {
                        (activity_type, attributes.link_attribute_types(&attribute_types))
                    })
                    .collect();

                Self { name: value.name, entities, agents, activities, roles: value.roles }
            }
        }

        let domain = ChronicleDomain::from_path(path)?;
        Ok(domain.into())
    }

    pub fn generate_domain_collections(&self) -> Result<Vec<Collection>, ChronicleSynthError> {
        let mut collections = vec![self.generate_roles()?];
        collections.extend(self.generate_activity_schema()?);
        collections.extend(self.generate_agent_schema()?);
        collections.extend(self.generate_entity_schema()?);
        Ok(collections)
    }

    fn generate_roles(&self) -> Result<Collection, ChronicleSynthError> {
        generate_roles(&self.roles)
    }

    fn generate_activity_schema(&self) -> Result<Vec<Collection>, ChronicleSynthError> {
        generate_schema(&self.activities)
    }

    fn generate_agent_schema(&self) -> Result<Vec<Collection>, ChronicleSynthError> {
        generate_schema(&self.agents)
    }

    fn generate_entity_schema(&self) -> Result<Vec<Collection>, ChronicleSynthError> {
        generate_schema(&self.entities)
    }
}

#[derive(Debug, Deserialize, Eq, PartialEq, Hash, PartialOrd, Ord)]
struct AttributeType(String);

#[derive(Debug, Deserialize, Eq, PartialEq, Hash, PartialOrd, Ord)]
struct ParsedDomainType(String);

impl ParsedDomainType {
    fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Deserialize)]
struct Role(String);

impl Role {
    fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug)]
enum SynthType {
    String,
    Object,
    Number,
    Bool,
}

impl From<&ChroniclePrimitive> for SynthType {
    fn from(value: &ChroniclePrimitive) -> Self {
        match value.r#type {
            PrimitiveType::String => SynthType::String,
            PrimitiveType::JSON => SynthType::Object,
            PrimitiveType::Int => SynthType::Number,
            PrimitiveType::Bool => SynthType::Bool,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ChroniclePrimitive {
    #[serde(skip)]
    _doc: Option<String>,
    #[serde(rename = "type")]
    r#type: PrimitiveType,
}

#[derive(Debug, Deserialize)]
struct Attributes {
    #[serde(skip)]
    _doc: Option<String>,
    attributes: Vec<AttributeType>,
}

impl Attributes {
    fn link_attribute_types(
        self,
        attribute_types: &BTreeMap<AttributeType, ChroniclePrimitive>,
    ) -> BTreeMap<AttributeType, SynthType> {
        let mut attr = BTreeMap::new();
        for attr_type in self.attributes {
            let r#type: SynthType = attribute_types.get(&attr_type).unwrap().into();
            attr.insert(attr_type, r#type);
        }
        attr
    }
}

fn generate_roles(roles: &[Role]) -> Result<Collection, ChronicleSynthError> {
    let mut variants = vec![json!({
		"type": "string",
		"constant": "UNSPECIFIED"
	})];

    // Uppercase guaranteed by the Linter
    for role in roles {
        variants.push(json!({
			"type": "string",
			"constant": role.as_str()
		}));
    }

    let roles = json!({
		"type": "one_of",
		"variants": variants
	});

    let domain_collection = DomainCollection::new("roles", roles);

    Ok(Collection::Generator(Generator::DomainCollection(domain_collection)))
}

fn domain_type_id_for_domain(ParsedDomainType(r#type): &ParsedDomainType) -> Collection {
    let domain_type_id = json!({
		"type": "string",
		"constant": r#type
	});

    let collection_name = format!("{}_domain_type_id", r#type.to_lowercase());
    let domain_collection = DomainCollection::new(collection_name, domain_type_id);

    Collection::Generator(Generator::DomainCollection(domain_collection))
}

fn set_attributes(type_name_lower: &str) -> Collection {
    let type_collection = format!("@{}_attributes", type_name_lower);
    let type_domain_type = format!("@{}_domain_type_id", type_name_lower);
    let type_attributes = json!({
		"type": "object",
		"@id": "_:n1",
		"@type": {
			"type": "array",
			"length": 1,
			"content": "http://chronicle.works/chronicleoperations/ns#SetAttributes"
		},
		"http://chronicle.works/chronicleoperations/ns#activityName": "@activity_name",
		"http://chronicle.works/chronicleoperations/ns#attributes": {
			"type": "array",
			"length": 1,
			"content": {
				"type": "object",
				"@type": {
					"type": "string",
					"constant": "@json"
				},
				"@value": type_collection
			}
		},
		"http://chronicle.works/chronicleoperations/ns#domaintypeId": {
			"type": "array",
			"length": 1,
			"content": {
				"type": "object",
				"@value": type_domain_type
			}
		},
		"http://chronicle.works/chronicleoperations/ns#namespaceName": "@same_namespace_name",
		"http://chronicle.works/chronicleoperations/ns#namespaceUuid": "@same_namespace_uuid"
	});

    let name = format!("set_{}_attributes", type_name_lower);
    let domain_collection = DomainCollection::new(name, type_attributes);
    Collection::Operation(Operation::DomainCollection(domain_collection))
}

fn type_attribute_variants(
    type_name_lower: &str,
    attributes: &BTreeMap<AttributeType, SynthType>,
) -> Result<Collection, ChronicleSynthError> {
    let mut type_attribute_variants: BTreeMap<String, serde_json::Value> = maplit::btreemap! {
		"type".to_string() => json!("object"),
	};

    for (AttributeType(attribute), r#type) in attributes {
        let type_attribute_variant = match r#type {
            SynthType::String => {
                json!({
					"type": "string",
					"faker": {
						"generator": "bs_noun"
					}
				})
            }
            SynthType::Number => {
                json!({
					"type": "number",
					"subtype": "u32"
				})
            }
            SynthType::Bool => {
                json!({
					"type": "bool",
					"frequency": 0.5
				})
            }
            // Object will be an empty object.
            // This is something that could be tweaked on a case by case basis given some domain
            // knowledge
            SynthType::Object => {
                json!({
					"type": "object",
				})
            }
        };

        type_attribute_variants.insert(attribute.clone(), type_attribute_variant);
    }

    let name = format!("{}_attributes", type_name_lower);
    let schema = serde_json::to_value(type_attribute_variants)?;
    let collection = DomainCollection::new(name, schema);

    Ok(Collection::Generator(Generator::DomainCollection(collection)))
}

fn generate_schema(
    types_attributes: &BTreeMap<ParsedDomainType, BTreeMap<AttributeType, SynthType>>,
) -> Result<Vec<Collection>, ChronicleSynthError> {
    let mut collections = Vec::new();

    for (r#type, attributes) in types_attributes {
        let collection1 = domain_type_id_for_domain(r#type);
        collections.push(collection1);

        let type_name_lower = r#type.as_str().to_lowercase();

        let collection2 = set_attributes(&type_name_lower);
        collections.push(collection2);

        let collection3 = type_attribute_variants(&type_name_lower, attributes)?;
        collections.push(collection3);
    }
    Ok(collections)
}

#[cfg(test)]
mod tests {
    use maplit::btreemap;

    use crate::collection::CollectionHandling;

    use super::*;

    #[test]
    fn test_type_attribute_variants() {
        // Create a sample attribute map with two attributes
        let attributes = btreemap! {
			AttributeType("TestAttribute1".to_owned()) => SynthType::String,
			AttributeType("TestAttribute2".to_owned()) => SynthType::Number,
		};

        // Call the function being tested
        let result = type_attribute_variants("test_type", &attributes).unwrap();

        // Assert that the function returns a Collection with the expected properties
        insta::assert_json_snapshot!(result.json_schema().unwrap().to_string(), @r###""{\"TestAttribute1\":{\"faker\":{\"generator\":\"bs_noun\"},\"type\":\"string\"},\"TestAttribute2\":{\"subtype\":\"u32\",\"type\":\"number\"},\"type\":\"object\"}""###);
    }
}
