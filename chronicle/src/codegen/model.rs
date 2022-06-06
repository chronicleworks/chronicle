use std::{collections::HashMap, path::Path, str::FromStr};

use inflector::cases::kebabcase::to_kebab_case;
use inflector::cases::pascalcase::to_pascal_case;
use inflector::cases::snakecase::to_snake_case;
use inflector::string::singularize::to_singular;
use serde::{Deserialize, Serialize};

custom_error::custom_error! {pub ModelError
    AttributeNotDefined{attr: String} = "Attribute not defined",
    ModelFileNotReadable{source: std::io::Error} = "Model file not readable",
    ModelFileInvalidJson{source: serde_json::Error} = "Model file invalid JSON",
    ModelFileInvalidYaml{source: serde_yaml::Error} = "Model file invalid YAML",
    ParseDomainError = "Domain not parsable",
    SerializeJsonError = "Model not serializable to JSON",
    SerializeYamlError = "Model not serializable to YAML",
}

#[derive(Deserialize, Serialize, Debug, Copy, Clone, PartialEq, Eq)]
pub enum PrimitiveType {
    String,
    Bool,
    Int,
}

#[derive(Debug, Clone, Serialize)]
pub struct AttributeDef {
    typ: String,
    pub primitive_type: PrimitiveType,
}

impl TypeName for AttributeDef {
    fn as_type_name(&self) -> String {
        to_pascal_case(&to_singular(&self.typ))
    }
}

impl AttributeDef {
    pub fn as_scalar_type(&self) -> String {
        to_pascal_case(&format!("{}Attribute", self.as_type_name()))
    }

    pub fn as_property(&self) -> String {
        to_snake_case(&to_singular(&format!("{}Attribute", self.typ)))
    }
}

/// A name formatted for CLI use - kebab-case, singular, lowercase
pub trait CliName {
    fn as_cli_name(&self) -> String;
}

/// A correctly cased and singularized name for the type
pub trait TypeName {
    fn as_type_name(&self) -> String;
}

/// Entities, Activites and Agents have a specific set of attributes.
pub trait AttributesTypeName {
    fn attributes_type_name(&self) -> String;
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
}

impl<T> CliName for T
where
    T: TypeName,
{
    fn as_cli_name(&self) -> String {
        to_kebab_case(&*self.as_type_name())
    }
}

impl<T> Property for T
where
    T: TypeName,
{
    fn as_property(&self) -> String {
        to_snake_case(&*self.as_type_name())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentDef {
    pub(crate) name: String,
    pub attributes: Vec<AttributeDef>,
}

impl TypeName for &AgentDef {
    fn as_type_name(&self) -> String {
        to_pascal_case(&to_singular(&self.name))
    }
}

impl AgentDef {
    pub fn new(name: impl AsRef<str>, attributes: Vec<AttributeDef>) -> Self {
        Self {
            name: name.as_ref().to_string(),
            attributes,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EntityDef {
    pub(crate) name: String,
    pub attributes: Vec<AttributeDef>,
}

impl TypeName for &EntityDef {
    fn as_type_name(&self) -> String {
        to_pascal_case(&to_singular(&self.name))
    }
}

impl EntityDef {
    pub fn new(name: impl AsRef<str>, attributes: Vec<AttributeDef>) -> Self {
        Self {
            name: name.as_ref().to_string(),
            attributes,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivityDef {
    pub(crate) name: String,
    pub attributes: Vec<AttributeDef>,
}

impl TypeName for &ActivityDef {
    fn as_type_name(&self) -> String {
        to_pascal_case(&to_singular(&self.name))
    }
}

impl ActivityDef {
    pub fn new(name: impl AsRef<str>, attributes: Vec<AttributeDef>) -> Self {
        Self {
            name: name.as_ref().to_string(),
            attributes,
        }
    }
}

pub struct AgentBuilder<'a>(&'a ChronicleDomainDef, AgentDef);

impl<'a> AgentBuilder<'a> {
    pub fn new(domain: &'a ChronicleDomainDef, name: impl AsRef<str>) -> Self {
        Self(domain, AgentDef::new(name, vec![]))
    }

    pub fn with_attribute(mut self, typ: impl AsRef<str>) -> Result<Self, ModelError> {
        let attr = self
            .0
            .attribute(typ.as_ref())
            .ok_or(ModelError::AttributeNotDefined {
                attr: typ.as_ref().to_string(),
            })?;
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
    pub fn new(domain: &'a ChronicleDomainDef, name: impl AsRef<str>) -> Self {
        Self(domain, EntityDef::new(name, vec![]))
    }

    pub fn with_attribute(mut self, typ: impl AsRef<str>) -> Result<Self, ModelError> {
        let attr = self
            .0
            .attribute(typ.as_ref())
            .ok_or(ModelError::AttributeNotDefined {
                attr: typ.as_ref().to_string(),
            })?;
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
    pub fn new(domain: &'a ChronicleDomainDef, name: impl AsRef<str>) -> Self {
        Self(domain, ActivityDef::new(name, vec![]))
    }

    pub fn with_attribute(mut self, typ: impl AsRef<str>) -> Result<Self, ModelError> {
        let attr = self
            .0
            .attribute(typ.as_ref())
            .ok_or(ModelError::AttributeNotDefined {
                attr: typ.as_ref().to_string(),
            })?;
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
        Builder(ChronicleDomainDef {
            name: name.as_ref().to_string(),
            agents: vec![],
            entities: vec![],
            activities: vec![],
            attributes: vec![],
        })
    }

    pub fn with_attribute_type(
        mut self,
        name: impl AsRef<str>,
        typ: PrimitiveType,
    ) -> Result<Self, ModelError> {
        self.0.attributes.push(AttributeDef {
            typ: name.as_ref().to_string(),
            primitive_type: typ,
        });

        Ok(self)
    }

    pub fn with_agent(
        mut self,
        name: impl AsRef<str>,
        b: impl FnOnce(AgentBuilder<'_>) -> Result<AgentBuilder<'_>, ModelError>,
    ) -> Result<Self, ModelError> {
        self.0
            .agents
            .push(b(AgentBuilder(&self.0, AgentDef::new(name, vec![])))?.into());
        Ok(self)
    }

    pub fn with_entity(
        mut self,
        name: impl AsRef<str>,
        b: impl FnOnce(EntityBuilder<'_>) -> Result<EntityBuilder<'_>, ModelError>,
    ) -> Result<Self, ModelError> {
        self.0
            .entities
            .push(b(EntityBuilder(&self.0, EntityDef::new(name, vec![])))?.into());
        Ok(self)
    }

    pub fn with_activity(
        mut self,
        name: impl AsRef<str>,
        b: impl FnOnce(ActivityBuilder<'_>) -> Result<ActivityBuilder<'_>, ModelError>,
    ) -> Result<Self, ModelError> {
        self.0
            .activities
            .push(b(ActivityBuilder(&self.0, ActivityDef::new(name, vec![])))?.into());

        Ok(self)
    }

    pub fn build(self) -> ChronicleDomainDef {
        self.0
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct AttributeFileInput {
    pub typ: PrimitiveType,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct DomainFileInput {
    pub name: String,
    pub attributes: HashMap<String, AttributeFileInput>,
    pub agents: HashMap<String, HashMap<String, AttributeFileInput>>,
    pub entities: HashMap<String, HashMap<String, AttributeFileInput>>,
    pub activities: HashMap<String, HashMap<String, AttributeFileInput>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChronicleDomainDef {
    name: String,
    pub attributes: Vec<AttributeDef>,
    pub agents: Vec<AgentDef>,
    pub entities: Vec<EntityDef>,
    pub activities: Vec<ActivityDef>,
}

impl ChronicleDomainDef {
    pub fn attribute(&self, attr: &str) -> Option<AttributeDef> {
        self.attributes.iter().find(|a| a.typ == attr).cloned()
    }

    fn from_json(file: &str) -> Result<Self, ModelError> {
        match serde_json::from_str::<DomainFileInput>(file) {
            Err(source) => Err(ModelError::ModelFileInvalidJson { source }),
            Ok(model) => Self::from_model(model),
        }
    }

    fn from_yaml(file: &str) -> Result<Self, ModelError> {
        match serde_yaml::from_str::<DomainFileInput>(file) {
            Err(source) => Err(ModelError::ModelFileInvalidYaml { source }),
            Ok(model) => Self::from_model(model),
        }
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ModelError> {
        let path = path.as_ref();
        let file: String = std::fs::read_to_string(&path)?;
        match path.extension().and_then(|s| s.to_str()) {
            Some("json") => Self::from_json(&file),
            _ => Self::from_yaml(&file),
        }
    }

    fn from_model(model: DomainFileInput) -> Result<Self, ModelError> {
        let mut builder = Builder::new(model.name);

        for (name, attr) in model.attributes {
            builder = builder.with_attribute_type(name, attr.typ)?;
        }

        for (name, attributes) in model.agents {
            for (attr, _) in attributes {
                builder = builder.with_agent(&name, |agent| agent.with_attribute(attr))?;
            }
        }

        for (name, attributes) in model.entities {
            for (attr, _) in attributes {
                builder = builder.with_entity(&name, |entity| entity.with_attribute(attr))?;
            }
        }

        for (name, attributes) in model.activities {
            for (attr, _) in attributes {
                builder = builder.with_activity(&name, |activity| activity.with_attribute(attr))?;
            }
        }

        Ok(builder.build())
    }

    fn to_string(&self) -> Result<String, ModelError> {
        if let Ok(s) = serde_json::to_string(&self) {
            Ok(s)
        } else {
            Err(ModelError::SerializeJsonError)
        }
    }

    fn to_json_string(&self) -> Result<String, ModelError> {
        if let Ok(s) = serde_json::to_string(&self) {
            Ok(s)
        } else {
            Err(ModelError::SerializeJsonError)
        }
    }

    fn to_yaml_string(&self) -> Result<String, ModelError> {
        if let Ok(s) = serde_yaml::to_string(&self) {
            Ok(s)
        } else {
            Err(ModelError::SerializeYamlError)
        }
    }
}

impl FromStr for ChronicleDomainDef {
    type Err = ModelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match Self::from_yaml(s) {
            Err(_) => match Self::from_json(s) {
                Err(_) => Err(ModelError::ParseDomainError),
                Ok(domain) => Ok(domain),
            },
            Ok(domain) => Ok(domain),
        }
    }
}

#[cfg(test)]
pub mod test {
    use super::{ChronicleDomainDef, EntityDef};

    use std::cmp::Ordering;

    impl PartialEq for EntityDef {
        fn eq(&self, other: &Self) -> bool {
            self.name == other.name
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
            self.name.cmp(&other.name)
        }
    }

    use assert_fs::prelude::*;

    #[test]
    fn json_from_file() -> Result<(), Box<dyn std::error::Error>> {
        let file = assert_fs::NamedTempFile::new("test.json")?;
        file.write_str(
            r#" {
            "name": "chronicle",
            "attributes": {
              "stringAttribute": {
                "typ": "String"
              }
            },
            "agents": {
              "friend": {
                "stringAttribute": {
                  "typ": "String"
                }
              }
            },
            "entities": {
              "octopi": {
                "stringAttribute": {
                  "typ": "String"
                }
              },
              "the sea": {
                "stringAttribute": {
                  "typ": "String"
                }
              }
            },
            "activities": {
              "gardening": {
                "stringAttribute": {
                  "typ": "String"
                }
              }
            }
          }
         "#,
        )?;

        let mut domain = ChronicleDomainDef::from_file(&file.path()).unwrap();

        domain.entities.sort();

        insta::assert_debug_snapshot!(domain);

        Ok(())
    }

    #[test]
    fn yaml_from_file() -> Result<(), Box<dyn std::error::Error>> {
        let file = assert_fs::NamedTempFile::new("test.yml")?;
        file.write_str(
            r#"
        name: "test"
        attributes:
          stringAttribute:
            typ: "String"
        agents:
          friend:
            stringAttribute:
              typ: "String"
        entities:
          octopi:
            stringAttribute:
              typ: "String"
          the sea:
            stringAttribute:
              typ: "String"
        activities:
          gardening:
            stringAttribute:
              typ: "String"
         "#,
        )?;

        let mut domain = ChronicleDomainDef::from_file(&file.path()).unwrap();

        domain.entities.sort();

        insta::assert_debug_snapshot!(domain);

        Ok(())
    }

    use std::str::FromStr;

    #[test]
    fn test_from_str() -> Result<(), Box<dyn std::error::Error>> {
        let file = assert_fs::NamedTempFile::new("test.yml")?;
        file.write_str(
            r#"
        name: "test"
        attributes:
          stringAttribute:
            typ: "String"
        agents:
          friend:
            stringAttribute:
              typ: "String"
        entities:
          octopi:
            stringAttribute:
              typ: "String"
          the sea:
            stringAttribute:
              typ: "String"
        activities:
          gardening:
            stringAttribute:
              typ: "String"
         "#,
        )?;

        let s: String = std::fs::read_to_string(&file.path())?;

        let mut domain = ChronicleDomainDef::from_str(&s)?;

        domain.entities.sort();
        insta::assert_debug_snapshot!(domain);

        Ok(())
    }

    #[test]
    fn test_to_json_string() -> Result<(), Box<dyn std::error::Error>> {
        let s = r#"
        name: "test"
        attributes:
          stringAttribute:
            typ: "String"
        agents:
          friend:
            stringAttribute:
              typ: "String"
        entities:
          octopi:
            stringAttribute:
              typ: "String"
        activities:
          gardening:
            stringAttribute:
              typ: "String"
         "#
        .to_string();

        let domain = ChronicleDomainDef::from_str(&s)?;
        eprintln!("{}", domain.to_json_string().unwrap());
        insta::assert_debug_snapshot!(format!("{}", domain.to_json_string().unwrap()));

        Ok(())
    }

    #[test]
    fn test_to_yaml_string() -> Result<(), Box<dyn std::error::Error>> {
        let s = r#"
        name: "test"
        attributes:
          stringAttribute:
            typ: "String"
        agents:
          friend:
            stringAttribute:
              typ: "String"
        entities:
          octopi:
            stringAttribute:
              typ: "String"
        activities:
          gardening:
            stringAttribute:
              typ: "String"
         "#
        .to_string();

        let domain = ChronicleDomainDef::from_str(&s)?;
        eprintln!("{}", domain.to_yaml_string().unwrap());
        insta::assert_debug_snapshot!(format!("{}", domain.to_yaml_string().unwrap()));

        Ok(())
    }

    #[test]
    fn test_to_string() -> Result<(), Box<dyn std::error::Error>> {
        let s = r#"
        name: "test"
        attributes:
          stringAttribute:
            typ: "String"
        agents:
          friend:
            stringAttribute:
              typ: "String"
        entities:
          octopi:
            stringAttribute:
              typ: "String"
        activities:
          gardening:
            stringAttribute:
              typ: "String"
         "#
        .to_string();

        let domain = ChronicleDomainDef::from_str(&s)?;
        eprintln!("{}", domain.to_string().unwrap());
        insta::assert_debug_snapshot!(format!("{}", domain.to_string().unwrap()));

        Ok(())
    }
}
