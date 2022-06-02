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

    pub fn from_input(name: String, attributes: HashMap<String, AttributeFileInput>) -> Self {
        let mut v = Vec::new();
        for (attr, attr_def) in attributes {
            let a: AttributeDef = AttributeDef::from_attribute_file_input(attr, attr_def);
            v.push(a);
        }
        AgentDef {
            name,
            attributes: v,
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

    pub fn from_input(name: String, attributes: HashMap<String, AttributeFileInput>) -> Self {
        let mut v = Vec::new();
        for (attr, attr_def) in attributes {
            let a: AttributeDef = AttributeDef::from_attribute_file_input(attr, attr_def);
            v.push(a);
        }
        EntityDef {
            name,
            attributes: v,
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

    pub fn from_input(name: String, attributes: HashMap<String, AttributeFileInput>) -> Self {
        let mut v = Vec::new();
        for (attr, attr_def) in attributes {
            let a: AttributeDef = AttributeDef::from_attribute_file_input(attr, attr_def);
            v.push(a);
        }
        ActivityDef {
            name,
            attributes: v,
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

impl From<&AttributeDef> for AttributeFileInput {
    fn from(attr: &AttributeDef) -> Self {
        Self {
            typ: attr.primitive_type,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct DomainFileInput {
    pub name: String,
    pub attributes: HashMap<String, AttributeFileInput>,
    pub agents: HashMap<String, HashMap<String, AttributeFileInput>>,
    pub entities: HashMap<String, HashMap<String, AttributeFileInput>>,
    pub activities: HashMap<String, HashMap<String, AttributeFileInput>>,
}

impl DomainFileInput {
    pub fn new(name: impl AsRef<str>) -> Self {
        DomainFileInput {
            name: name.as_ref().to_string(),
            attributes: HashMap::new(),
            agents: HashMap::new(),
            entities: HashMap::new(),
            activities: HashMap::new(),
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

        for agent in &domain.agents {
            let name = &agent.name;
            let mut input_attributes = HashMap::new();
            for attr in &agent.attributes {
                let name = attr.typ.to_string();
                input_attributes.insert(name, attr.into());
            }
            file.agents.insert(name.to_string(), input_attributes);
        }

        for entity in &domain.entities {
            let name = &entity.name;
            let mut input_attributes = HashMap::new();
            for attr in &entity.attributes {
                let name = attr.typ.to_string();
                input_attributes.insert(name, attr.into());
            }
            file.entities.insert(name.to_string(), input_attributes);
        }

        for activity in &domain.activities {
            let name = &activity.name;
            let mut input_attributes = HashMap::new();
            for attr in &activity.attributes {
                let name = attr.typ.to_string();
                input_attributes.insert(name, AttributeFileInput::from(attr));
            }
            file.activities.insert(name.to_string(), input_attributes);
        }

        file
    }
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
        Self::from_str(&file)
    }

    fn from_model(model: DomainFileInput) -> Result<Self, ModelError> {
        let mut builder = Builder::new(model.name);

        for (name, attr) in model.attributes {
            builder = builder.with_attribute_type(name, attr.typ)?;
        }

        for (name, attributes) in model.agents {
            builder
                .0
                .agents
                .push(AgentDef::from_input(name, attributes))
        }

        for (name, attributes) in model.entities {
            builder
                .0
                .entities
                .push(EntityDef::from_input(name, attributes))
        }

        for (name, attributes) in model.activities {
            builder
                .0
                .activities
                .push(ActivityDef::from_input(name, attributes))
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
      string:
        typ: "String"
      int:
        typ: "Int"
      bool:
        typ: "Bool"
    agents:
      friends:
        string:
          typ: "String"
        int:
          typ: "Int"
        bool:
          typ: "Bool"
    entities:
      octopi:
        string:
          typ: "String"
        int:
          typ: "Int"
        bool:
          typ: "Bool"
      the sea:
        string:
          typ: "String"
        int:
          typ: "Int"
        bool:
          typ: "Bool"
    activities:
      gardening:
        string:
          typ: "String"
        int:
          typ: "Int"
        bool:
          typ: "Bool"
      swim about:
        string:
          typ: "String"
        int:
          typ: "Int"
        bool:
          typ: "Bool"
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
          string:
            typ: "String"
        agents:
          friend:
            string:
              typ: "String"
        entities:
          octopi:
            string:
              typ: "String"
        activities:
          gardening:
            string:
              typ: "String"
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
                  "string": {
                    "typ": "String"
                  }
                },
                "agents": {
                  "friend": {
                    "string": {
                      "typ": "String"
                    }
                  }
                },
                "entities": {
                  "octopi": {
                    "string": {
                      "typ": "String"
                    }
                  },
                  "the sea": {
                    "string": {
                      "typ": "String"
                    }
                  }
                },
                "activities": {
                  "gardening": {
                    "string": {
                      "typ": "String"
                    }
                  }
                }
              }
             "#,
        )?;
        Ok(file)
    }

    #[test]
    fn from_str_for_domain_input() -> Result<(), Box<dyn std::error::Error>> {
        let s = r#"
        name: "chronicle"
        attributes:
          string:
            typ: "String"
          int:
            typ: "Int"
          bool:
            typ: "Bool"
        agents:
          friends:
            string:
              typ: "String"
            int:
              typ: "Int"
            bool:
              typ: "Bool"
        entities:
          octopi:
            string:
              typ: "String"
            int:
              typ: "Int"
            bool:
              typ: "Bool"
          the sea:
            string:
              typ: "String"
            int:
              typ: "Int"
            bool:
              typ: "Bool"
        activities:
          gardening:
            string:
              typ: "String"
            int:
              typ: "Int"
            bool:
              typ: "Bool"
          swim about:
            string:
              typ: "String"
            int:
              typ: "Int"
            bool:
              typ: "Bool"
         "#;
        let input = DomainFileInput::from_str(s);

        insta::assert_debug_snapshot!(input);

        Ok(())
    }

    #[test]
    fn json_from_file() -> Result<(), Box<dyn std::error::Error>> {
        let file = create_test_json_file()?;

        let mut domain = ChronicleDomainDef::from_file(&file.path()).unwrap();

        domain.entities.sort();

        insta::assert_debug_snapshot!(domain);

        Ok(())
    }

    #[test]
    fn yaml_from_file() -> Result<(), Box<dyn std::error::Error>> {
        let file = create_test_yaml_file()?;

        let mut domain = ChronicleDomainDef::from_file(&file.path()).unwrap();

        domain.entities.sort();

        insta::assert_debug_snapshot!(domain);

        Ok(())
    }

    use std::str::FromStr;

    #[test]
    fn test_chronicle_domain_def_from_str() -> Result<(), Box<dyn std::error::Error>> {
        let file = create_test_yaml_file()?;
        let s: String = std::fs::read_to_string(&file.path())?;
        let mut domain = ChronicleDomainDef::from_str(&s)?;

        domain.entities.sort();
        insta::assert_debug_snapshot!(domain);

        Ok(())
    }

    #[test]
    fn test_from_domain_for_file_input() -> Result<(), Box<dyn std::error::Error>> {
        let file = create_test_yaml_file_single_entity()?;
        let s: String = std::fs::read_to_string(&file.path())?;
        let domain = ChronicleDomainDef::from_str(&s)?;
        let input = DomainFileInput::from(&domain);

        insta::assert_debug_snapshot!(input);

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
        insta::assert_debug_snapshot!(input);
    }

    #[test]
    fn test_to_json_string() -> Result<(), Box<dyn std::error::Error>> {
        let file = create_test_yaml_file_single_entity()?;
        let s: String = std::fs::read_to_string(&file.path())?;
        let domain = ChronicleDomainDef::from_str(&s)?;

        insta::assert_debug_snapshot!(domain.to_json_string().unwrap());

        Ok(())
    }

    #[test]
    fn test_to_yaml_string() -> Result<(), Box<dyn std::error::Error>> {
        let file = create_test_yaml_file_single_entity()?;
        let s: String = std::fs::read_to_string(&file.path())?;
        let domain = ChronicleDomainDef::from_str(&s)?;

        insta::assert_debug_snapshot!(domain.to_yaml_string().unwrap());

        Ok(())
    }
}
