use std::{collections::HashMap, path::Path};

use inflector::cases::pascalcase::to_pascal_case;
use inflector::cases::snakecase::to_snake_case;
use inflector::string::singularize::to_singular;
use serde::{Deserialize, Serialize};

custom_error::custom_error! {pub ModelError
    AttributeNotDefined{attr: String} = "Attribute not defined",
    FileExtensionInvalid = "Invalid path extension",
    FileExtensionNotReadable = "JSON or YAML path extension not readable",
    ModelFileNotReadable{source: std::io::Error} = "Model file not readable",
    ModelFileInvalidYaml{source: serde_yaml::Error} = "Model file invalid YAML",
    ModelFileInvalidJson{source: serde_json::Error} = "Model file invalid JSON",
}

#[derive(Deserialize, Serialize, Debug, Copy, Clone, PartialEq, Eq)]
pub enum PrimitiveType {
    String,
    Bool,
    Int,
}

#[derive(Debug, Clone)]
pub struct AttributeDef {
    typ: String,
    pub primitive_type: PrimitiveType,
}

impl AttributeDef {
    pub fn as_type_name(&self) -> String {
        to_pascal_case(&to_singular(&self.typ))
    }

    pub fn as_scalar_type(&self) -> String {
        to_pascal_case(&format!("{}Attribute", self.as_type_name()))
    }

    pub fn as_property(&self) -> String {
        to_snake_case(&to_singular(&format!("{}Attribute", self.typ)))
    }
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

impl<T> Property for T
where
    T: TypeName,
{
    fn as_property(&self) -> String {
        to_snake_case(&*self.as_type_name())
    }
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ModelError> {
        let path = path.as_ref();
        let file: String = std::fs::read_to_string(&path)?;
        let model = {
            match path.extension().and_then(|s| s.to_str()) {
                Some("json") => match serde_json::from_str::<DomainFileInput>(&file) {
                    Err(source) => return Err(ModelError::ModelFileInvalidJson { source }),
                    Ok(result) => result,
                },
                _ => match serde_yaml::from_str::<DomainFileInput>(&file) {
                    Err(source) => return Err(ModelError::ModelFileInvalidYaml { source }),
                    Ok(result) => result,
                },
            }
        };
        Self::from_file_model(model)
    }

    fn from_file_model(model: DomainFileInput) -> Result<Self, ModelError> {
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
}

#[cfg(test)]
pub mod test {
    use super::{ChronicleDomainDef, DomainFileInput, EntityDef};

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

    #[test]
    pub fn from_json() {
        let json = r#" {
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
         "#;

        let mut domain = ChronicleDomainDef::from_file_model(
            serde_json::from_str::<DomainFileInput>(json).unwrap(),
        )
        .unwrap();

        domain.entities.sort();

        insta::assert_debug_snapshot!(domain);
    }

    #[test]
    pub fn from_yaml() {
        let yaml = r#"
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
         "#;

        let mut domain = ChronicleDomainDef::from_file_model(
            serde_yaml::from_str::<DomainFileInput>(yaml).unwrap(),
        )
        .unwrap();

        domain.entities.sort();

        insta::assert_debug_snapshot!(domain);
    }
}
