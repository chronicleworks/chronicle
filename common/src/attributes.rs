use std::collections::HashMap;

use serde_json::Value;

use crate::prov::DomaintypeId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Attribute {
    pub typ: String,
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Attributes {
    pub typ: Option<DomaintypeId>,
    pub attributes: HashMap<String, Attribute>,
}
