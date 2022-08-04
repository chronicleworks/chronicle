use std::{collections::BTreeMap, path::Path, str::FromStr};

use inflector::{
    cases::{kebabcase::to_kebab_case, pascalcase::to_pascal_case, snakecase::to_snake_case},
    string::singularize::to_singular,
};
use serde::{Deserialize, Serialize};

custom_error::custom_error! {pub ModelError
    AttributeNotDefined{attr: String} = "Attribute not defined",
    ModelFileNotReadable{source: std::io::Error} = "Model file not readable",
    ModelFileInvalidJson{source: serde_json::Error} = "Model file invalid JSON",
    ModelFileInvalidYaml{source: serde_yaml::Error} = "Model file invalid YAML",
}

#[derive(Deserialize, Serialize, Debug, Copy, Clone, PartialEq, Eq)]
pub enum PrimitiveType {
    String,
    Bool,
    Int,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

    pub fn from_attribute_file_input(name: String, attr: AttributeFileInput) -> Self {
        AttributeDef {
            typ: name,
            primitive_type: attr.typ,
        }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

    pub fn from_input<'a>(
        name: String,
        attributes: &BTreeMap<String, AttributeFileInput>,
        attribute_references: impl Iterator<Item = &'a AttributeRef>,
    ) -> Result<Self, ModelError> {
        Ok(Self {
            name,
            attributes: attribute_references
                .map(|x| {
                    attributes
                        .get(&*x.0)
                        .ok_or_else(|| ModelError::AttributeNotDefined {
                            attr: x.0.to_owned(),
                        })
                        .map(|attr| AttributeDef {
                            typ: x.0.to_owned(),
                            primitive_type: attr.typ,
                        })
                })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

    pub fn from_input<'a>(
        name: String,
        attributes: &BTreeMap<String, AttributeFileInput>,
        attribute_references: impl Iterator<Item = &'a AttributeRef>,
    ) -> Result<Self, ModelError> {
        Ok(Self {
            name,
            attributes: attribute_references
                .map(|x| {
                    attributes
                        .get(&*x.0)
                        .ok_or_else(|| ModelError::AttributeNotDefined {
                            attr: x.0.to_owned(),
                        })
                        .map(|attr| AttributeDef {
                            typ: x.0.to_owned(),
                            primitive_type: attr.typ,
                        })
                })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

    pub fn from_input<'a>(
        name: String,
        attributes: &BTreeMap<String, AttributeFileInput>,
        attribute_references: impl Iterator<Item = &'a AttributeRef>,
    ) -> Result<Self, ModelError> {
        Ok(Self {
            name,
            attributes: attribute_references
                .map(|x| {
                    attributes
                        .get(&*x.0)
                        .ok_or_else(|| ModelError::AttributeNotDefined {
                            attr: x.0.to_owned(),
                        })
                        .map(|attr| AttributeDef {
                            typ: x.0.to_owned(),
                            primitive_type: attr.typ,
                        })
                })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleDef {
    pub(crate) name: String,
}

impl RoleDef {
    pub fn new(name: impl AsRef<str>) -> Self {
        Self {
            name: name.as_ref().to_string(),
        }
    }

    pub fn from_role_file_input(name: String) -> Self {
        RoleDef { name }
    }
}

impl TypeName for &RoleDef {
    fn as_type_name(&self) -> String {
        to_pascal_case(&to_singular(&self.name))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChronicleDomainDef {
    name: String,
    pub(crate) attributes: Vec<AttributeDef>,
    pub(crate) agents: Vec<AgentDef>,
    pub(crate) entities: Vec<EntityDef>,
    pub(crate) activities: Vec<ActivityDef>,
    pub(crate) roles: Vec<RoleDef>,
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
            ..Default::default()
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

    pub fn with_role(mut self, name: impl AsRef<str>) -> Result<Self, ModelError> {
        self.0.roles.push(RoleDef::new(name));

        Ok(self)
    }

    pub fn build(self) -> ChronicleDomainDef {
        self.0
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct AttributeFileInput {
    #[serde(rename = "type")]
    typ: PrimitiveType,
}

impl From<&AttributeDef> for AttributeFileInput {
    fn from(attr: &AttributeDef) -> Self {
        Self {
            typ: attr.primitive_type,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]

pub struct AttributeRef(String);

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct ResourceDef {
    pub attributes: Vec<AttributeRef>,
}

impl From<&AgentDef> for ResourceDef {
    fn from(agent: &AgentDef) -> Self {
        Self {
            attributes: agent
                .attributes
                .iter()
                .map(|attr| AttributeRef(attr.as_type_name()))
                .collect(),
        }
    }
}

impl From<&EntityDef> for ResourceDef {
    fn from(entity: &EntityDef) -> Self {
        Self {
            attributes: entity
                .attributes
                .iter()
                .map(|attr| AttributeRef(attr.as_type_name()))
                .collect(),
        }
    }
}
impl From<&ActivityDef> for ResourceDef {
    fn from(activity: &ActivityDef) -> Self {
        Self {
            attributes: activity
                .attributes
                .iter()
                .map(|attr| AttributeRef(attr.as_type_name()))
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
    pub roles: Vec<String>,
}

impl DomainFileInput {
    pub fn new(name: impl AsRef<str>) -> Self {
        DomainFileInput {
            name: name.as_ref().to_string(),
            ..Default::default()
        }
    }
}

impl FromStr for DomainFileInput {
    type Err = ModelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match serde_json::from_str::<DomainFileInput>(s) {
            Err(_) => match serde_yaml::from_str::<DomainFileInput>(s) {
                Err(source) => Err(ModelError::ModelFileInvalidYaml { source }),
                Ok(domain) => Ok(domain),
            },
            Ok(domain) => Ok(domain),
        }
    }
}

impl From<&ChronicleDomainDef> for DomainFileInput {
    fn from(domain: &ChronicleDomainDef) -> Self {
        let mut file = Self::new(&domain.name);

        for attr in &domain.attributes {
            let name = attr.typ.to_string();
            file.attributes.insert(name, attr.into());
        }

        file.agents = domain
            .agents
            .iter()
            .map(|x| (x.as_type_name(), ResourceDef::from(x)))
            .collect();

        file.entities = domain
            .entities
            .iter()
            .map(|x| (x.as_type_name(), ResourceDef::from(x)))
            .collect();

        file.activities = domain
            .activities
            .iter()
            .map(|x| (x.as_type_name(), ResourceDef::from(x)))
            .collect();

        file.roles = domain.roles.iter().map(|x| x.as_type_name()).collect();

        file
    }
}

impl ChronicleDomainDef {
    pub fn build(name: &str) -> Builder {
        Builder::new(name)
    }

    pub fn attribute(&self, attr: &str) -> Option<AttributeDef> {
        self.attributes.iter().find(|a| a.typ == attr).cloned()
    }

    pub fn from_input_string(s: &str) -> Result<Self, ModelError> {
        ChronicleDomainDef::from_str(s)
    }

    pub fn from_json(file: &str) -> Result<Self, ModelError> {
        match serde_json::from_str::<DomainFileInput>(file) {
            Err(source) => Err(ModelError::ModelFileInvalidJson { source }),
            Ok(model) => Self::from_model(model),
        }
    }

    pub fn from_yaml(file: &str) -> Result<Self, ModelError> {
        match serde_yaml::from_str::<DomainFileInput>(file) {
            Err(source) => Err(ModelError::ModelFileInvalidYaml { source }),
            Ok(model) => Self::from_model(model),
        }
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ModelError> {
        let path = path.as_ref();

        let file: String = std::fs::read_to_string(&path)?;

        match path.extension() {
            Some(ext) if ext == "json" => Self::from_json(&*file),
            _ => Self::from_yaml(&*file),
        }
    }

    fn from_model(model: DomainFileInput) -> Result<Self, ModelError> {
        let mut builder = Builder::new(model.name);

        for (name, attr) in model.attributes.iter() {
            builder = builder.with_attribute_type(name, attr.typ)?;
        }

        for (name, def) in model.agents {
            builder.0.agents.push(AgentDef::from_input(
                name,
                &model.attributes,
                def.attributes.iter(),
            )?)
        }

        for (name, def) in model.entities {
            builder.0.entities.push(EntityDef::from_input(
                name,
                &model.attributes,
                def.attributes.iter(),
            )?)
        }

        for (name, def) in model.activities {
            builder.0.activities.push(ActivityDef::from_input(
                name,
                &model.attributes,
                def.attributes.iter(),
            )?)
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

    pub fn to_yaml_string(&self) -> Result<String, ModelError> {
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

        let mut domain = ChronicleDomainDef::from_file(&file.path()).unwrap();

        domain.entities.sort();

        insta::assert_yaml_snapshot!(domain, @r###"
        ---
        name: chronicle
        attributes:
          - typ: String
            primitive_type: String
        agents:
          - name: friend
            attributes:
              - typ: String
                primitive_type: String
        entities:
          - name: octopi
            attributes:
              - typ: String
                primitive_type: String
          - name: the sea
            attributes:
              - typ: String
                primitive_type: String
        activities:
          - name: gardening
            attributes:
              - typ: String
                primitive_type: String
        roles:
          - name: drummer
        "###);

        Ok(())
    }

    #[test]
    fn yaml_from_file() -> Result<(), Box<dyn std::error::Error>> {
        let file = create_test_yaml_file()?;

        let mut domain = ChronicleDomainDef::from_file(&file.path()).unwrap();

        domain.entities.sort();

        insta::assert_yaml_snapshot!(domain, @r###"
        ---
        name: chronicle
        attributes:
          - typ: Bool
            primitive_type: Bool
          - typ: Int
            primitive_type: Int
          - typ: String
            primitive_type: String
        agents:
          - name: friends
            attributes:
              - typ: String
                primitive_type: String
              - typ: Int
                primitive_type: Int
              - typ: Bool
                primitive_type: Bool
        entities:
          - name: octopi
            attributes:
              - typ: String
                primitive_type: String
              - typ: Int
                primitive_type: Int
              - typ: Bool
                primitive_type: Bool
          - name: the sea
            attributes:
              - typ: String
                primitive_type: String
              - typ: Int
                primitive_type: Int
              - typ: Bool
                primitive_type: Bool
        activities:
          - name: gardening
            attributes:
              - typ: String
                primitive_type: String
              - typ: Int
                primitive_type: Int
              - typ: Bool
                primitive_type: Bool
          - name: swim about
            attributes:
              - typ: String
                primitive_type: String
              - typ: Int
                primitive_type: Int
              - typ: Bool
                primitive_type: Bool
        roles:
          - name: drummer
        "###);

        Ok(())
    }

    use std::str::FromStr;

    #[test]
    fn test_chronicle_domain_def_from_str() -> Result<(), Box<dyn std::error::Error>> {
        let file = create_test_yaml_file()?;
        let s: String = std::fs::read_to_string(&file.path())?;
        let domain = ChronicleDomainDef::from_str(&s)?;

        insta::assert_yaml_snapshot!(domain, @r###"
        ---
        name: chronicle
        attributes:
          - typ: Bool
            primitive_type: Bool
          - typ: Int
            primitive_type: Int
          - typ: String
            primitive_type: String
        agents:
          - name: friends
            attributes:
              - typ: String
                primitive_type: String
              - typ: Int
                primitive_type: Int
              - typ: Bool
                primitive_type: Bool
        entities:
          - name: octopi
            attributes:
              - typ: String
                primitive_type: String
              - typ: Int
                primitive_type: Int
              - typ: Bool
                primitive_type: Bool
          - name: the sea
            attributes:
              - typ: String
                primitive_type: String
              - typ: Int
                primitive_type: Int
              - typ: Bool
                primitive_type: Bool
        activities:
          - name: gardening
            attributes:
              - typ: String
                primitive_type: String
              - typ: Int
                primitive_type: Int
              - typ: Bool
                primitive_type: Bool
          - name: swim about
            attributes:
              - typ: String
                primitive_type: String
              - typ: Int
                primitive_type: Int
              - typ: Bool
                primitive_type: Bool
        roles:
          - name: drummer
        "###);

        Ok(())
    }

    #[test]
    fn test_from_domain_for_file_input() -> Result<(), Box<dyn std::error::Error>> {
        let file = create_test_yaml_file_single_entity()?;
        let s: String = std::fs::read_to_string(&file.path())?;
        let domain = ChronicleDomainDef::from_str(&s)?;
        let input = DomainFileInput::from(&domain);

        insta::assert_yaml_snapshot!(input, @r###"
        ---
        name: test
        attributes:
          String:
            type: String
        agents:
          Friend:
            attributes:
              - String
        entities:
          Octopus:
            attributes:
              - String
        activities:
          Gardening:
            attributes:
              - String
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
            primitive_type: PrimitiveType::String,
        };
        let input = AttributeFileInput::from(&attr);
        insta::assert_yaml_snapshot!(input, @r###"
        ---
        type: String
        "###);
    }

    #[test]
    fn test_to_json_string() -> Result<(), Box<dyn std::error::Error>> {
        let file = create_test_yaml_file_single_entity()?;
        let s: String = std::fs::read_to_string(&file.path())?;
        let domain = ChronicleDomainDef::from_str(&s)?;

        insta::assert_yaml_snapshot!(domain, @r###"
        ---
        name: test
        attributes:
          - typ: String
            primitive_type: String
        agents:
          - name: friend
            attributes:
              - typ: String
                primitive_type: String
        entities:
          - name: octopi
            attributes:
              - typ: String
                primitive_type: String
        activities:
          - name: gardening
            attributes:
              - typ: String
                primitive_type: String
        roles:
          - name: drummer
        "###);

        Ok(())
    }

    #[test]
    fn test_to_yaml_string() -> Result<(), Box<dyn std::error::Error>> {
        let file = create_test_yaml_file_single_entity()?;
        let s: String = std::fs::read_to_string(&file.path())?;
        let domain = ChronicleDomainDef::from_str(&s)?;

        insta::assert_yaml_snapshot!(domain, @r###"
        ---
        name: test
        attributes:
          - typ: String
            primitive_type: String
        agents:
          - name: friend
            attributes:
              - typ: String
                primitive_type: String
        entities:
          - name: octopi
            attributes:
              - typ: String
                primitive_type: String
        activities:
          - name: gardening
            attributes:
              - typ: String
                primitive_type: String
        roles:
          - name: drummer
        "###);

        Ok(())
    }
}
