use parity_scale_codec::{Decode, Encode};
use scale_info::{build::Fields, Path, Type, TypeInfo};
use serde_json::Value;
use std::collections::BTreeMap;

use crate::prov::DomaintypeId;

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct SerdeWrapper(pub Value);

impl std::fmt::Display for SerdeWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match serde_json::to_string(&self.0) {
            Ok(json_string) => write!(f, "{}", json_string),
            Err(e) => {
                tracing::error!("Failed to serialize Value to JSON string: {}", e);
                Err(std::fmt::Error)
            }
        }
    }
}

impl Encode for SerdeWrapper {
    fn encode_to<T: parity_scale_codec::Output + ?Sized>(&self, dest: &mut T) {
        let json_string =
            serde_json::to_string(&self.0).expect("Failed to serialize Value to JSON string");
        json_string.encode_to(dest);
    }
}

impl From<Value> for SerdeWrapper {
    fn from(value: Value) -> Self {
        SerdeWrapper(value)
    }
}

impl From<SerdeWrapper> for Value {
    fn from(wrapper: SerdeWrapper) -> Self {
        wrapper.0
    }
}

impl Decode for SerdeWrapper {
    fn decode<I: parity_scale_codec::Input>(
        input: &mut I,
    ) -> Result<Self, parity_scale_codec::Error> {
        let json_string = String::decode(input)?;
        let value = serde_json::from_str(&json_string).map_err(|_| {
            parity_scale_codec::Error::from("Failed to deserialize JSON string to Value")
        })?;
        Ok(SerdeWrapper(value))
    }
}

impl TypeInfo for SerdeWrapper {
    type Identity = Self;
    fn type_info() -> Type {
        Type::builder()
            .path(Path::new("SerdeWrapper", module_path!()))
            .composite(Fields::unnamed().field(|f| f.ty::<String>().type_name("Json")))
    }
}

#[derive(Debug, Clone, Encode, Decode, TypeInfo, Serialize, Deserialize, PartialEq, Eq)]
pub struct Attribute {
    pub typ: String,
    pub value: SerdeWrapper,
}

impl std::fmt::Display for Attribute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Type: {}, Value: {}",
            self.typ,
            serde_json::to_string(&self.value.0).unwrap_or_else(|_| String::from("Invalid Value"))
        )
    }
}

impl Attribute {
    pub fn get_type(&self) -> &String {
        &self.typ
    }

    pub fn get_value(&self) -> &Value {
        &self.value.0
    }
    pub fn new(typ: impl AsRef<str>, value: Value) -> Self {
        Self {
            typ: typ.as_ref().to_owned(),
            value: value.into(),
        }
    }
}

#[derive(
    Debug, Clone, Serialize, Deserialize, Encode, Decode, TypeInfo, PartialEq, Eq, Default,
)]
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
