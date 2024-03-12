use std::{collections::BTreeMap, path::Path, str::FromStr};

use inflector::cases::{
	camelcase::to_camel_case, kebabcase::to_kebab_case, pascalcase::to_pascal_case,
	snakecase::to_snake_case,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ModelError {
	#[error("Attribute not defined argument: {attr}")]
	AttributeNotDefined { attr: String },

	#[error("Model file not readable: {0}")]
	ModelFileNotReadable(
		#[from]
		#[source]
		std::io::Error,
	),

	#[error("Model file invalid JSON: {0}")]
	ModelFileInvalidJson(
		#[from]
		#[source]
		serde_json::Error,
	),

	#[error("Model file invalid YAML: {0}")]
	ModelFileInvalidYaml(
		#[from]
		#[source]
		serde_yaml::Error,
	),
}

#[derive(Deserialize, Serialize, Debug, Copy, Clone, PartialEq, Eq)]
pub enum PrimitiveType {
	String,
	Bool,
	Int,
	JSON,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeDef {
	typ: String,
	pub doc: Option<String>,
	pub primitive_type: PrimitiveType,
}

impl TypeName for AttributeDef {
	fn as_type_name(&self) -> String {
		to_pascal_case(&self.typ)
	}

	fn preserve_inflection(&self) -> String {
		match (self.typ.chars().next(), self.typ.chars().nth(1), &self.typ[1..]) {
			(_, Some(c), _) if c.is_uppercase() => format!("{}Attribute", self.typ),
			(Some(first), _, body) => format!("{}{}Attribute", first.to_lowercase(), body),
			_ => format!("{}Attribute", self.typ),
		}
	}
}

impl AttributeDef {
	pub fn as_scalar_type(&self) -> String {
		match (self.typ.chars().next(), self.typ.chars().nth(1), &self.typ[1..]) {
			(_, Some(c), _) if c.is_uppercase() => format!("{}Attribute", self.typ),
			(Some(first), _, body) => format!("{}{}Attribute", first.to_uppercase(), body),
			_ => format!("{}Attribute", self.as_type_name()),
		}
	}

	pub fn as_property(&self) -> String {
		to_snake_case(&format!("{}Attribute", self.typ))
	}

	pub fn from_attribute_file_input(external_id: String, attr: AttributeFileInput) -> Self {
		AttributeDef { typ: external_id, doc: attr.doc, primitive_type: attr.typ }
	}
}

/// A external_id formatted for CLI use - kebab-case, singular, lowercase

pub trait CliName {
	fn as_cli_name(&self) -> String;
}

/// A correctly cased and singularized external_id for the type
pub trait TypeName {
	fn as_type_name(&self) -> String;
	fn preserve_inflection(&self) -> String;

	fn as_method_name(&self) -> String {
		format!("define{}", self.as_type_name())
	}
}

/// Entities, Activities and Agents have a specific set of attributes.
pub trait AttributesTypeName {
	fn attributes_type_name(&self) -> String;
	fn attributes_type_name_preserve_inflection(&self) -> String;
}

pub trait Property {
	fn as_property(&self) -> String;
}

impl<T> AttributesTypeName for T
where
	T: TypeName,
{
	fn attributes_type_name(&self) -> String {
		to_pascal_case(&format!("{}Attributes", self.as_type_name()))
	}

	fn attributes_type_name_preserve_inflection(&self) -> String {
		format!("{}Attributes", self.as_type_name())
	}
}

impl<T> CliName for T
where
	T: TypeName,
{
	fn as_cli_name(&self) -> String {
		to_kebab_case(&self.as_type_name())
	}
}

impl<T> Property for T
where
	T: TypeName,
{
	fn as_property(&self) -> String {
		to_snake_case(&self.as_type_name())
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDef {
	pub external_id: String,
	pub doc: Option<String>,
	pub attributes: Vec<AttributeDef>,
}

impl TypeName for AgentDef {
	fn as_type_name(&self) -> String {
		type_name_for_kind("Agent", &self.external_id)
	}

	fn preserve_inflection(&self) -> String {
		preserve_inflection_for_kind("Agent", &self.external_id)
	}
}

impl AgentDef {
	pub fn new(
		external_id: impl AsRef<str>,
		doc: Option<String>,
		attributes: Vec<AttributeDef>,
	) -> Self {
		Self { external_id: external_id.as_ref().to_string(), doc, attributes }
	}

	pub fn from_input<'a>(
		external_id: String,
		doc: Option<String>,
		attributes: &BTreeMap<String, AttributeFileInput>,
		attribute_references: impl Iterator<Item = &'a AttributeRef>,
	) -> Result<Self, ModelError> {
		Ok(Self {
			external_id,
			doc,
			attributes: attribute_references
				.map(|x| {
					attributes
						.get(&*x.0)
						.ok_or_else(|| ModelError::AttributeNotDefined { attr: x.0.to_owned() })
						.map(|attr| AttributeDef {
							typ: x.0.to_owned(),
							doc: attr.doc.to_owned(),
							primitive_type: attr.typ,
						})
				})
				.collect::<Result<Vec<_>, _>>()?,
		})
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityDef {
	pub external_id: String,
	pub doc: Option<String>,
	pub attributes: Vec<AttributeDef>,
}

impl TypeName for EntityDef {
	fn as_type_name(&self) -> String {
		type_name_for_kind("Entity", &self.external_id)
	}

	fn preserve_inflection(&self) -> String {
		preserve_inflection_for_kind("Entity", &self.external_id)
	}
}

impl EntityDef {
	pub fn new(
		external_id: impl AsRef<str>,
		doc: Option<String>,
		attributes: Vec<AttributeDef>,
	) -> Self {
		Self { external_id: external_id.as_ref().to_string(), doc, attributes }
	}

	pub fn from_input<'a>(
		external_id: String,
		doc: Option<String>,
		attributes: &BTreeMap<String, AttributeFileInput>,
		attribute_references: impl Iterator<Item = &'a AttributeRef>,
	) -> Result<Self, ModelError> {
		Ok(Self {
			external_id,
			doc,
			attributes: attribute_references
				.map(|x| {
					attributes
						.get(&*x.0)
						.ok_or_else(|| ModelError::AttributeNotDefined { attr: x.0.to_owned() })
						.map(|attr| AttributeDef {
							typ: x.0.to_owned(),
							doc: attr.doc.to_owned(),
							primitive_type: attr.typ,
						})
				})
				.collect::<Result<Vec<_>, _>>()?,
		})
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityDef {
	pub external_id: String,
	pub doc: Option<String>,
	pub attributes: Vec<AttributeDef>,
}

impl TypeName for ActivityDef {
	fn as_type_name(&self) -> String {
		type_name_for_kind("Activity", &self.external_id)
	}

	fn preserve_inflection(&self) -> String {
		preserve_inflection_for_kind("Activity", &self.external_id)
	}
}

impl ActivityDef {
	pub fn new(
		external_id: impl AsRef<str>,
		doc: Option<String>,
		attributes: Vec<AttributeDef>,
	) -> Self {
		Self { external_id: external_id.as_ref().to_string(), doc, attributes }
	}

	pub fn from_input<'a>(
		external_id: String,
		doc: Option<String>,
		attributes: &BTreeMap<String, AttributeFileInput>,
		attribute_references: impl Iterator<Item = &'a AttributeRef>,
	) -> Result<Self, ModelError> {
		Ok(Self {
			external_id,
			doc,
			attributes: attribute_references
				.map(|x| {
					attributes
						.get(&*x.0)
						.ok_or_else(|| ModelError::AttributeNotDefined { attr: x.0.to_owned() })
						.map(|attr| AttributeDef {
							typ: x.0.to_owned(),
							doc: attr.doc.to_owned(),
							primitive_type: attr.typ,
						})
				})
				.collect::<Result<Vec<_>, _>>()?,
		})
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleDef {
	pub external_id: String,
}

impl RoleDef {
	pub fn new(external_id: impl AsRef<str>) -> Self {
		Self { external_id: external_id.as_ref().to_string() }
	}

	pub fn from_role_file_input(external_id: String) -> Self {
		RoleDef { external_id }
	}
}

impl TypeName for &RoleDef {
	fn as_type_name(&self) -> String {
		to_pascal_case(&self.external_id)
	}

	fn preserve_inflection(&self) -> String {
		self.external_id.clone()
	}
}

fn type_name_for_kind(kind: &str, id: &str) -> String {
	if id == format!("Prov{kind}") {
		id.to_string()
	} else {
		match (id.chars().next(), id.chars().nth(1), &id[1..]) {
			(_, Some(c), _) if c.is_uppercase() => format!("{id}{kind}"),
			(Some(first), _, body) => format!("{}{}{}", first.to_uppercase(), body, kind),
			_ => format!("{}{}", to_pascal_case(id), kind),
		}
	}
}

fn preserve_inflection_for_kind(kind: &str, id: &str) -> String {
	match (id.chars().next(), id.chars().nth(1), &id[1..]) {
		(_, Some(c), _) if c.is_uppercase() => format!("{id}{kind}"),
		(Some(first), _, body) => format!("{}{}{}", first.to_lowercase(), body, kind),
		_ => to_camel_case(&format!("{id}{kind}")),
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChronicleDomainDef {
	name: String,
	pub attributes: Vec<AttributeDef>,
	pub agents: Vec<AgentDef>,
	pub entities: Vec<EntityDef>,
	pub activities: Vec<ActivityDef>,
	pub roles_doc: Option<String>,
	pub roles: Vec<RoleDef>,
}

pub struct AgentBuilder<'a>(&'a ChronicleDomainDef, AgentDef);

impl<'a> AgentBuilder<'a> {
	pub fn new(
		domain: &'a ChronicleDomainDef,
		external_id: impl AsRef<str>,
		doc: Option<String>,
	) -> Self {
		Self(domain, AgentDef::new(external_id, doc, vec![]))
	}

	pub fn with_attribute(mut self, typ: impl AsRef<str>) -> Result<Self, ModelError> {
		let attr = self
			.0
			.attribute(typ.as_ref())
			.ok_or(ModelError::AttributeNotDefined { attr: typ.as_ref().to_string() })?;
		self.1.attributes.push(attr);
		Ok(self)
	}
}

impl<'a> From<AgentBuilder<'a>> for AgentDef {
	fn from(val: AgentBuilder<'a>) -> Self {
		val.1
	}
}

pub struct EntityBuilder<'a>(&'a ChronicleDomainDef, EntityDef);

impl<'a> EntityBuilder<'a> {
	pub fn new(
		domain: &'a ChronicleDomainDef,
		external_id: impl AsRef<str>,
		doc: Option<String>,
	) -> Self {
		Self(domain, EntityDef::new(external_id, doc, vec![]))
	}

	pub fn with_attribute(mut self, typ: impl AsRef<str>) -> Result<Self, ModelError> {
		let attr = self
			.0
			.attribute(typ.as_ref())
			.ok_or(ModelError::AttributeNotDefined { attr: typ.as_ref().to_string() })?;
		self.1.attributes.push(attr);
		Ok(self)
	}
}

impl<'a> From<EntityBuilder<'a>> for EntityDef {
	fn from(val: EntityBuilder<'a>) -> Self {
		val.1
	}
}

pub struct ActivityBuilder<'a>(&'a ChronicleDomainDef, ActivityDef);

impl<'a> ActivityBuilder<'a> {
	pub fn new(
		domain: &'a ChronicleDomainDef,
		external_id: impl AsRef<str>,
		doc: Option<String>,
	) -> Self {
		Self(domain, ActivityDef::new(external_id, doc, vec![]))
	}

	pub fn with_attribute(mut self, typ: impl AsRef<str>) -> Result<Self, ModelError> {
		let attr = self
			.0
			.attribute(typ.as_ref())
			.ok_or(ModelError::AttributeNotDefined { attr: typ.as_ref().to_string() })?;
		self.1.attributes.push(attr);
		Ok(self)
	}
}

impl<'a> From<ActivityBuilder<'a>> for ActivityDef {
	fn from(val: ActivityBuilder<'a>) -> Self {
		val.1
	}
}

pub struct Builder(ChronicleDomainDef);

impl Builder {
	pub fn new(name: impl AsRef<str>) -> Self {
		Builder(ChronicleDomainDef { name: name.as_ref().to_string(), ..Default::default() })
	}

	pub fn with_attribute_type(
		mut self,
		external_id: impl AsRef<str>,
		doc: Option<String>,
		typ: PrimitiveType,
	) -> Result<Self, ModelError> {
		self.0.attributes.push(AttributeDef {
			typ: external_id.as_ref().to_string(),
			doc,
			primitive_type: typ,
		});

		Ok(self)
	}

	pub fn with_agent(
		mut self,
		external_id: impl AsRef<str>,
		doc: Option<String>,
		b: impl FnOnce(AgentBuilder<'_>) -> Result<AgentBuilder<'_>, ModelError>,
	) -> Result<Self, ModelError> {
		self.0
			.agents
			.push(b(AgentBuilder(&self.0, AgentDef::new(external_id, doc, vec![])))?.into());
		Ok(self)
	}

	pub fn with_entity(
		mut self,
		external_id: impl AsRef<str>,
		doc: Option<String>,
		b: impl FnOnce(EntityBuilder<'_>) -> Result<EntityBuilder<'_>, ModelError>,
	) -> Result<Self, ModelError> {
		self.0
			.entities
			.push(b(EntityBuilder(&self.0, EntityDef::new(external_id, doc, vec![])))?.into());
		Ok(self)
	}

	pub fn with_activity(
		mut self,
		external_id: impl AsRef<str>,
		doc: Option<String>,
		b: impl FnOnce(ActivityBuilder<'_>) -> Result<ActivityBuilder<'_>, ModelError>,
	) -> Result<Self, ModelError> {
		self.0
			.activities
			.push(b(ActivityBuilder(&self.0, ActivityDef::new(external_id, doc, vec![])))?.into());

		Ok(self)
	}

	pub fn with_role(mut self, external_id: impl AsRef<str>) -> Result<Self, ModelError> {
		self.0.roles.push(RoleDef::new(external_id));

		Ok(self)
	}

	pub fn build(self) -> ChronicleDomainDef {
		self.0
	}
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct AttributeFileInput {
	doc: Option<String>,
	#[serde(rename = "type")]
	typ: PrimitiveType,
}

impl From<&AttributeDef> for AttributeFileInput {
	fn from(attr: &AttributeDef) -> Self {
		Self { doc: attr.doc.to_owned(), typ: attr.primitive_type }
	}
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]

pub struct AttributeRef(pub String);

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct ResourceDef {
	pub doc: Option<String>,
	pub attributes: Vec<AttributeRef>,
}

impl From<&AgentDef> for ResourceDef {
	fn from(agent: &AgentDef) -> Self {
		Self {
			doc: agent.doc.to_owned(),
			attributes: agent
				.attributes
				.iter()
				.map(|attr| AttributeRef(attr.typ.to_owned()))
				.collect(),
		}
	}
}

impl From<&EntityDef> for ResourceDef {
	fn from(entity: &EntityDef) -> Self {
		Self {
			doc: entity.doc.to_owned(),
			attributes: entity
				.attributes
				.iter()
				.map(|attr| AttributeRef(attr.typ.to_owned()))
				.collect(),
		}
	}
}

impl From<&ActivityDef> for ResourceDef {
	fn from(activity: &ActivityDef) -> Self {
		Self {
			doc: activity.doc.to_owned(),
			attributes: activity
				.attributes
				.iter()
				.map(|attr| AttributeRef(attr.typ.to_owned()))
				.collect(),
		}
	}
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct DomainFileInput {
	pub name: String,
	pub attributes: BTreeMap<String, AttributeFileInput>,
	pub agents: BTreeMap<String, ResourceDef>,
	pub entities: BTreeMap<String, ResourceDef>,
	pub activities: BTreeMap<String, ResourceDef>,
	pub roles_doc: Option<String>,
	pub roles: Vec<String>,
}

impl DomainFileInput {
	pub fn new(name: impl AsRef<str>) -> Self {
		DomainFileInput { name: name.as_ref().to_string(), ..Default::default() }
	}
}

impl FromStr for DomainFileInput {
	type Err = ModelError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match serde_json::from_str::<DomainFileInput>(s) {
			Err(_) => Ok(serde_yaml::from_str::<DomainFileInput>(s)?),
			Ok(domain) => Ok(domain),
		}
	}
}

impl From<&ChronicleDomainDef> for DomainFileInput {
	fn from(domain: &ChronicleDomainDef) -> Self {
		let mut file = Self::new(&domain.name);

		for attr in &domain.attributes {
			let external_id = attr.typ.to_string();
			file.attributes.insert(external_id, attr.into());
		}

		file.agents = domain
			.agents
			.iter()
			.map(|x| (x.external_id.clone(), ResourceDef::from(x)))
			.collect();

		file.entities = domain
			.entities
			.iter()
			.map(|x| (x.external_id.clone(), ResourceDef::from(x)))
			.collect();

		file.activities = domain
			.activities
			.iter()
			.map(|x| (x.external_id.clone(), ResourceDef::from(x)))
			.collect();

		file.roles_doc = domain.roles_doc.to_owned();

		file.roles = domain.roles.iter().map(|x| x.as_type_name()).collect();

		file
	}
}

impl ChronicleDomainDef {
	pub fn build(external_id: &str) -> Builder {
		Builder::new(external_id)
	}

	fn attribute(&self, attr: &str) -> Option<AttributeDef> {
		self.attributes.iter().find(|a| a.typ == attr).cloned()
	}

	pub fn from_input_string(s: &str) -> Result<Self, ModelError> {
		ChronicleDomainDef::from_str(s)
	}

	fn from_json(file: &str) -> Result<Self, ModelError> {
		let model = serde_json::from_str::<DomainFileInput>(file)?;
		Self::from_model(model)
	}

	fn from_yaml(file: &str) -> Result<Self, ModelError> {
		let model = serde_yaml::from_str::<DomainFileInput>(file)?;
		Self::from_model(model)
	}

	pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ModelError> {
		let path = path.as_ref();

		let file: String = std::fs::read_to_string(path)?;

		match path.extension() {
			Some(ext) if ext == "json" => Self::from_json(&file),
			_ => Self::from_yaml(&file),
		}
	}

	fn from_model(model: DomainFileInput) -> Result<Self, ModelError> {
		let mut builder = Builder::new(model.name);

		for (external_id, attr) in model.attributes.iter() {
			builder = builder.with_attribute_type(external_id, attr.doc.to_owned(), attr.typ)?;
		}

		for (external_id, def) in model.agents {
			builder.0.agents.push(AgentDef::from_input(
				external_id,
				def.doc,
				&model.attributes,
				def.attributes.iter(),
			)?)
		}

		for (external_id, def) in model.entities {
			builder.0.entities.push(EntityDef::from_input(
				external_id,
				def.doc,
				&model.attributes,
				def.attributes.iter(),
			)?)
		}

		for (external_id, def) in model.activities {
			builder.0.activities.push(ActivityDef::from_input(
				external_id,
				def.doc,
				&model.attributes,
				def.attributes.iter(),
			)?)
		}

		if model.roles_doc.is_some() {
			builder.0.roles_doc = model.roles_doc;
		}

		for role in model.roles {
			builder.0.roles.push(RoleDef::from_role_file_input(role));
		}

		Ok(builder.build())
	}

	pub fn to_json_string(&self) -> Result<String, ModelError> {
		let input: DomainFileInput = self.into();
		let json = serde_json::to_string(&input)?;
		Ok(json)
	}

	fn to_yaml_string(&self) -> Result<String, ModelError> {
		let input: DomainFileInput = self.into();
		let yaml = serde_yaml::to_string(&input)?;
		Ok(yaml)
	}
}

/// Parse from a yaml formatted string
impl FromStr for ChronicleDomainDef {
	type Err = ModelError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Self::from_yaml(s)
	}
}

#[cfg(test)]
pub mod test {
	use super::{ChronicleDomainDef, DomainFileInput, EntityDef};

	use std::cmp::Ordering;

	impl PartialEq for EntityDef {
		fn eq(&self, other: &Self) -> bool {
			self.external_id == other.external_id
		}
	}

	impl Eq for EntityDef {}

	impl PartialOrd for EntityDef {
		fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
			Some(self.cmp(other))
		}
	}

	impl Ord for EntityDef {
		fn cmp(&self, other: &Self) -> Ordering {
			self.external_id.cmp(&other.external_id)
		}
	}

	use assert_fs::prelude::*;

	fn create_test_yaml_file() -> Result<assert_fs::NamedTempFile, Box<dyn std::error::Error>> {
		let file = assert_fs::NamedTempFile::new("test.yml")?;
		file.write_str(
			r#"
    name: "chronicle"
    attributes:
      String:
        type: "String"
      Int:
        type: "Int"
      Bool:
        type: "Bool"
    agents:
      friends:
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
      - drummer
     "#,
		)?;
		Ok(file)
	}

	// more than one entity will be in no particular order
	fn create_test_yaml_file_single_entity(
	) -> Result<assert_fs::NamedTempFile, Box<dyn std::error::Error>> {
		let file = assert_fs::NamedTempFile::new("test.yml")?;
		file.write_str(
			r#"
        name: "test"
        attributes:
          String:
            type: String
        agents:
          friend:
            attributes:
              - String
        entities:
          octopi:
            attributes:
              - String
        activities:
          gardening:
            attributes:
              - String
        roles:
          - drummer
         "#,
		)?;
		Ok(file)
	}

	fn create_test_json_file() -> Result<assert_fs::NamedTempFile, Box<dyn std::error::Error>> {
		let file = assert_fs::NamedTempFile::new("test.json")?;
		file.write_str(
			r#" {
                "name": "chronicle",
                "attributes": {
                  "String": {
                    "type": "String"
                  }
                },
                "agents": {
                  "friend": {
                    "attributes": [
                      "String"
                    ]
                  }
                },
                "entities": {
                  "octopi": {
                    "attributes": [
                      "String"
                    ]
                  },
                  "the sea": {
                    "attributes": [
                      "String"
                    ]
                  }
                },
                "activities": {
                  "gardening": {
                    "attributes": [
                      "String"
                    ]
                  }
                },
                "roles" : ["drummer"]
              }
             "#,
		)?;
		Ok(file)
	}

	#[test]
	fn json_from_file() -> Result<(), Box<dyn std::error::Error>> {
		let file = create_test_json_file()?;

		let mut domain = ChronicleDomainDef::from_file(file.path()).unwrap();

		domain.entities.sort();

		insta::assert_yaml_snapshot!(domain, @r###"
        ---
        name: chronicle
        attributes:
          - typ: String
            doc: ~
            primitive_type: String
        agents:
          - external_id: friend
            doc: ~
            attributes:
              - typ: String
                doc: ~
                primitive_type: String
        entities:
          - external_id: octopi
            doc: ~
            attributes:
              - typ: String
                doc: ~
                primitive_type: String
          - external_id: the sea
            doc: ~
            attributes:
              - typ: String
                doc: ~
                primitive_type: String
        activities:
          - external_id: gardening
            doc: ~
            attributes:
              - typ: String
                doc: ~
                primitive_type: String
        roles_doc: ~
        roles:
          - external_id: drummer
        "###);

		Ok(())
	}

	#[test]
	fn yaml_from_file() -> Result<(), Box<dyn std::error::Error>> {
		let file = create_test_yaml_file()?;

		let mut domain = ChronicleDomainDef::from_file(file.path()).unwrap();

		domain.entities.sort();

		insta::assert_yaml_snapshot!(domain, @r###"
        ---
        name: chronicle
        attributes:
          - typ: Bool
            doc: ~
            primitive_type: Bool
          - typ: Int
            doc: ~
            primitive_type: Int
          - typ: String
            doc: ~
            primitive_type: String
        agents:
          - external_id: friends
            doc: ~
            attributes:
              - typ: String
                doc: ~
                primitive_type: String
              - typ: Int
                doc: ~
                primitive_type: Int
              - typ: Bool
                doc: ~
                primitive_type: Bool
        entities:
          - external_id: octopi
            doc: ~
            attributes:
              - typ: String
                doc: ~
                primitive_type: String
              - typ: Int
                doc: ~
                primitive_type: Int
              - typ: Bool
                doc: ~
                primitive_type: Bool
          - external_id: the sea
            doc: ~
            attributes:
              - typ: String
                doc: ~
                primitive_type: String
              - typ: Int
                doc: ~
                primitive_type: Int
              - typ: Bool
                doc: ~
                primitive_type: Bool
        activities:
          - external_id: gardening
            doc: ~
            attributes:
              - typ: String
                doc: ~
                primitive_type: String
              - typ: Int
                doc: ~
                primitive_type: Int
              - typ: Bool
                doc: ~
                primitive_type: Bool
          - external_id: swim about
            doc: ~
            attributes:
              - typ: String
                doc: ~
                primitive_type: String
              - typ: Int
                doc: ~
                primitive_type: Int
              - typ: Bool
                doc: ~
                primitive_type: Bool
        roles_doc: ~
        roles:
          - external_id: drummer
        "###);

		Ok(())
	}

	use std::str::FromStr;

	#[test]
	fn test_chronicle_domain_def_from_str() -> Result<(), Box<dyn std::error::Error>> {
		let file = create_test_yaml_file()?;
		let s: String = std::fs::read_to_string(file.path())?;
		let domain = ChronicleDomainDef::from_str(&s)?;

		insta::assert_yaml_snapshot!(domain, @r###"
        ---
        name: chronicle
        attributes:
          - typ: Bool
            doc: ~
            primitive_type: Bool
          - typ: Int
            doc: ~
            primitive_type: Int
          - typ: String
            doc: ~
            primitive_type: String
        agents:
          - external_id: friends
            doc: ~
            attributes:
              - typ: String
                doc: ~
                primitive_type: String
              - typ: Int
                doc: ~
                primitive_type: Int
              - typ: Bool
                doc: ~
                primitive_type: Bool
        entities:
          - external_id: octopi
            doc: ~
            attributes:
              - typ: String
                doc: ~
                primitive_type: String
              - typ: Int
                doc: ~
                primitive_type: Int
              - typ: Bool
                doc: ~
                primitive_type: Bool
          - external_id: the sea
            doc: ~
            attributes:
              - typ: String
                doc: ~
                primitive_type: String
              - typ: Int
                doc: ~
                primitive_type: Int
              - typ: Bool
                doc: ~
                primitive_type: Bool
        activities:
          - external_id: gardening
            doc: ~
            attributes:
              - typ: String
                doc: ~
                primitive_type: String
              - typ: Int
                doc: ~
                primitive_type: Int
              - typ: Bool
                doc: ~
                primitive_type: Bool
          - external_id: swim about
            doc: ~
            attributes:
              - typ: String
                doc: ~
                primitive_type: String
              - typ: Int
                doc: ~
                primitive_type: Int
              - typ: Bool
                doc: ~
                primitive_type: Bool
        roles_doc: ~
        roles:
          - external_id: drummer
        "###);

		Ok(())
	}

	#[test]
	fn test_from_domain_for_file_input() -> Result<(), Box<dyn std::error::Error>> {
		let file = create_test_yaml_file_single_entity()?;
		let s: String = std::fs::read_to_string(file.path())?;
		let domain = ChronicleDomainDef::from_str(&s)?;
		let input = DomainFileInput::from(&domain);

		insta::assert_yaml_snapshot!(input, @r###"
        ---
        name: test
        attributes:
          String:
            doc: ~
            type: String
        agents:
          friend:
            doc: ~
            attributes:
              - String
        entities:
          octopi:
            doc: ~
            attributes:
              - String
        activities:
          gardening:
            doc: ~
            attributes:
              - String
        roles_doc: ~
        roles:
          - Drummer
        "###);

		Ok(())
	}

	use super::{AttributeDef, AttributeFileInput, PrimitiveType};

	#[test]
	fn test_from_attribute_def_for_attribute_file_input() {
		let attr = AttributeDef {
			typ: "string".to_string(),
			doc: None,
			primitive_type: PrimitiveType::String,
		};
		let input = AttributeFileInput::from(&attr);
		insta::assert_yaml_snapshot!(input, @r###"
        ---
        doc: ~
        type: String
        "###);
	}

	#[test]
	fn test_to_json_string() -> Result<(), Box<dyn std::error::Error>> {
		let file = create_test_yaml_file_single_entity()?;
		let s: String = std::fs::read_to_string(file.path())?;
		let domain = ChronicleDomainDef::from_str(&s)?;

		insta::assert_yaml_snapshot!(domain, @r###"
        ---
        name: test
        attributes:
          - typ: String
            doc: ~
            primitive_type: String
        agents:
          - external_id: friend
            doc: ~
            attributes:
              - typ: String
                doc: ~
                primitive_type: String
        entities:
          - external_id: octopi
            doc: ~
            attributes:
              - typ: String
                doc: ~
                primitive_type: String
        activities:
          - external_id: gardening
            doc: ~
            attributes:
              - typ: String
                doc: ~
                primitive_type: String
        roles_doc: ~
        roles:
          - external_id: drummer
        "###);

		Ok(())
	}

	#[test]
	fn test_to_yaml_string() -> Result<(), Box<dyn std::error::Error>> {
		let file = create_test_yaml_file_single_entity()?;
		let s: String = std::fs::read_to_string(file.path())?;
		let domain = ChronicleDomainDef::from_str(&s)?;

		insta::assert_yaml_snapshot!(domain, @r###"
        ---
        name: test
        attributes:
          - typ: String
            doc: ~
            primitive_type: String
        agents:
          - external_id: friend
            doc: ~
            attributes:
              - typ: String
                doc: ~
                primitive_type: String
        entities:
          - external_id: octopi
            doc: ~
            attributes:
              - typ: String
                doc: ~
                primitive_type: String
        activities:
          - external_id: gardening
            doc: ~
            attributes:
              - typ: String
                doc: ~
                primitive_type: String
        roles_doc: ~
        roles:
          - external_id: drummer
        "###);

		Ok(())
	}

	fn create_test_yaml_file_with_acronyms(
	) -> Result<assert_fs::NamedTempFile, Box<dyn std::error::Error>> {
		let file = assert_fs::NamedTempFile::new("test.yml")?;
		file.write_str(
			r#"
          name: "evidence"
          attributes:
            Content:
              type: String
            CMSId:
              type: String
            Title:
              type: String
            SearchParameter:
              type: String
            Reference:
              type: String
            Version:
              type: Int
          entities:
            Question:
              attributes:
                - CMSId
                - Content
            EvidenceReference:
              attributes:
                - SearchParameter
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
              attributes: []
            Published:
              attributes:
                - Version
            Revised:
              attributes:
                - CMSId
                - Version
          agents:
            Person:
              attributes:
                - CMSId
            Organization:
              attributes:
                - Title
          roles:
            - STAKEHOLDER
            - AUTHOR
            - RESEARCHER
            - EDITOR
   "#,
		)?;
		Ok(file)
	}

	#[test]
	fn test_from_domain_for_file_input_with_inflections() -> Result<(), Box<dyn std::error::Error>>
	{
		let file = create_test_yaml_file_with_acronyms()?;
		let s: String = std::fs::read_to_string(file.path())?;
		let domain = ChronicleDomainDef::from_str(&s)?;
		let input = DomainFileInput::from(&domain);

		insta::assert_yaml_snapshot!(input, @r###"
        ---
        name: evidence
        attributes:
          CMSId:
            doc: ~
            type: String
          Content:
            doc: ~
            type: String
          Reference:
            doc: ~
            type: String
          SearchParameter:
            doc: ~
            type: String
          Title:
            doc: ~
            type: String
          Version:
            doc: ~
            type: Int
        agents:
          Organization:
            doc: ~
            attributes:
              - Title
          Person:
            doc: ~
            attributes:
              - CMSId
        entities:
          EvidenceReference:
            doc: ~
            attributes:
              - SearchParameter
              - Reference
          Guidance:
            doc: ~
            attributes:
              - Title
              - Version
          PublishedGuidance:
            doc: ~
            attributes: []
          Question:
            doc: ~
            attributes:
              - CMSId
              - Content
        activities:
          Published:
            doc: ~
            attributes:
              - Version
          QuestionAsked:
            doc: ~
            attributes:
              - Content
          Researched:
            doc: ~
            attributes: []
          Revised:
            doc: ~
            attributes:
              - CMSId
              - Version
        roles_doc: ~
        roles:
          - Stakeholder
          - Author
          - Researcher
          - Editor
        "###);
		Ok(())
	}

	fn create_test_yaml_file_with_docs(
	) -> Result<assert_fs::NamedTempFile, Box<dyn std::error::Error>> {
		let file = assert_fs::NamedTempFile::new("test.yml")?;
		file.write_str(
            r#"
            name: Artworld
            attributes:
              Title:
                doc: |
                  # `Title`

                  `Title` can be the title attributed to

                  * `Artwork`
                  * `ArtworkDetails`
                  * `Created`

                type: String
              Location:
                type: String
              PurchaseValue:
                type: String
              PurchaseValueCurrency:
                type: String
              Description:
                type: String
              Name:
                type: String
            agents:
              Collector:
                doc: |
                  # `Collector`

                  Collectors purchase and amass collections of art.

                  Collectors might well be involved in exhibiting (`Exhibited`) and selling (`Sold`) works of art.
                attributes:
                  - Name
              Artist:
                doc: |
                  # `Artist`

                  Artists create new works of art.

                  Artists might well be involved in exhibiting (`Exhibited`) and selling (`Sold`) works of art.
                attributes:
                  - Name
            entities:
              Artwork:
                doc: |
                  # `Artwork`

                  Refers to the actual physical art piece.

                  ## Examples

                  This entity can be defined in Chronicle using GraphQL like so:

                  ```graphql
                  mutation {
                    defineArtworkEntity(
                      externalId: "salvatormundi"
                      attributes: { titleAttribute: "Salvator Mundi" }
                    ) {
                      context
                      txId
                    }
                  }
                  ```

                attributes:
                  - Title
              ArtworkDetails:
                doc: |
                  # `ArtworkDetails`

                  Provides more information about the piece, such as its title and description

                  ## Examples

                  This entity can be defined in Chronicle using GraphQL like so:

                  ```graphql
                  mutation {
                    defineArtworkDetailsEntity(
                      externalId: "salvatormundidetails"
                      attributes: {
                        titleAttribute: "Salvator Mundi"
                        descriptionAttribute: "Depiction of Christ holding a crystal orb and making the sign of the blessing."
                      }
                    ) {
                      context
                      txId
                    }
                  }
                  ```

                attributes:
                  - Title
                  - Description
            activities:
              Exhibited:
                attributes:
                  - Location
              Created:
                doc: |
                  # `Created`

                  `Created` refers to the act of creating a new piece of art
                attributes:
                  - Title
              Sold:
                attributes:
                  - PurchaseValue
                  - PurchaseValueCurrency
            roles_doc: |
              # Buyer, Seller, and Creator Roles

              ## Examples

              In the context of association with selling (`Sold`) of an `Artwork`,
              an `Artist`'s function could be `SELLER`, and `CREATOR` in the context
              of creation (`Created`).

              A `Collector`'s function in the context of the sale (`Sold`) of an
              `Artwork` could be `BUYER` or `SELLER`.
            roles:
              - BUYER
              - SELLER
              - CREATOR
   "#,
        )?;
		Ok(file)
	}

	#[test]
	fn test_from_domain_for_file_input_with_docs() -> Result<(), Box<dyn std::error::Error>> {
		let file = create_test_yaml_file_with_docs()?;
		let s: String = std::fs::read_to_string(file.path())?;
		let domain = ChronicleDomainDef::from_str(&s)?;
		let input = DomainFileInput::from(&domain);

		insta::assert_yaml_snapshot!(input, @r###"
        ---
        name: Artworld
        attributes:
          Description:
            doc: ~
            type: String
          Location:
            doc: ~
            type: String
          Name:
            doc: ~
            type: String
          PurchaseValue:
            doc: ~
            type: String
          PurchaseValueCurrency:
            doc: ~
            type: String
          Title:
            doc: "# `Title`\n\n`Title` can be the title attributed to\n\n* `Artwork`\n* `ArtworkDetails`\n* `Created`\n"
            type: String
        agents:
          Artist:
            doc: "# `Artist`\n\nArtists create new works of art.\n\nArtists might well be involved in exhibiting (`Exhibited`) and selling (`Sold`) works of art.\n"
            attributes:
              - Name
          Collector:
            doc: "# `Collector`\n\nCollectors purchase and amass collections of art.\n\nCollectors might well be involved in exhibiting (`Exhibited`) and selling (`Sold`) works of art.\n"
            attributes:
              - Name
        entities:
          Artwork:
            doc: "# `Artwork`\n\nRefers to the actual physical art piece.\n\n## Examples\n\nThis entity can be defined in Chronicle using GraphQL like so:\n\n```graphql\nmutation {\n  defineArtworkEntity(\n    externalId: \"salvatormundi\"\n    attributes: { titleAttribute: \"Salvator Mundi\" }\n  ) {\n    context\n    txId\n  }\n}\n```\n"
            attributes:
              - Title
          ArtworkDetails:
            doc: "# `ArtworkDetails`\n\nProvides more information about the piece, such as its title and description\n\n## Examples\n\nThis entity can be defined in Chronicle using GraphQL like so:\n\n```graphql\nmutation {\n  defineArtworkDetailsEntity(\n    externalId: \"salvatormundidetails\"\n    attributes: {\n      titleAttribute: \"Salvator Mundi\"\n      descriptionAttribute: \"Depiction of Christ holding a crystal orb and making the sign of the blessing.\"\n    }\n  ) {\n    context\n    txId\n  }\n}\n```\n"
            attributes:
              - Title
              - Description
        activities:
          Created:
            doc: "# `Created`\n\n`Created` refers to the act of creating a new piece of art\n"
            attributes:
              - Title
          Exhibited:
            doc: ~
            attributes:
              - Location
          Sold:
            doc: ~
            attributes:
              - PurchaseValue
              - PurchaseValueCurrency
        roles_doc: "# Buyer, Seller, and Creator Roles\n\n## Examples\n\nIn the context of association with selling (`Sold`) of an `Artwork`,\nan `Artist`'s function could be `SELLER`, and `CREATOR` in the context\nof creation (`Created`).\n\nA `Collector`'s function in the context of the sale (`Sold`) of an\n`Artwork` could be `BUYER` or `SELLER`.\n"
        roles:
          - Buyer
          - Seller
          - Creator
        "###);
		Ok(())
	}
}
