use std::collections::BTreeMap;

use serde_json::Value;

use crate::prov::DomaintypeId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Attribute {
    pub typ: String,
    pub value: Value,
}

impl Attribute {
    pub fn new(typ: impl AsRef<str>, value: Value) -> Self {
        Self {
            typ: typ.as_ref().to_owned(),
            value,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Attributes {
    pub typ: Option<DomaintypeId>,
    pub attributes: BTreeMap<String, Attribute>,
}

impl Attributes {
    pub fn type_only(typ: Option<DomaintypeId>) -> Self {
        Self {
            typ,
            attributes: BTreeMap::new(),
        }
    }
}
