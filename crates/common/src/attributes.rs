#[cfg(feature = "std")]
use std::collections::BTreeSet;

#[cfg(not(feature = "std"))]
use parity_scale_codec::{alloc::collections::BTreeSet, alloc::string::String, alloc::vec::Vec};
#[cfg(feature = "parity-encoding")]
use parity_scale_codec::Encode;
#[cfg(feature = "parity-encoding")]
use scale_encode::error::Kind;
#[cfg(not(feature = "std"))]
use scale_info::{prelude::borrow::ToOwned};
use serde_json::Value;

use crate::prov::DomaintypeId;

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct SerdeWrapper(pub Value);

impl core::fmt::Display for SerdeWrapper {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match serde_json::to_string(&self.0) {
            Ok(json_string) => write!(f, "{}", json_string),
            Err(e) => {
                tracing::error!("Failed to serialize Value to JSON string: {}", e);
                Err(core::fmt::Error)
            }
        }
    }
}

impl From<Value> for SerdeWrapper {
    fn from(value: Value) -> Self {
        SerdeWrapper(value)
    }
}

#[cfg(feature = "parity-encoding")]
impl scale_encode::EncodeAsType for SerdeWrapper {
    fn encode_as_type_to(
        &self,
        type_id: u32,
        _types: &scale_info::PortableRegistry,
        out: &mut scale_encode::Vec<u8>,
    ) -> Result<(), scale_encode::Error> {
        let json_string = match serde_json::to_string(&self.0) {
            Ok(json_string) => json_string,
            Err(e) => {
                tracing::error!("Failed to serialize Value to JSON string: {}", e);
                return Err(scale_encode::Error::new(scale_encode::error::ErrorKind::WrongShape {
                    actual: Kind::Str,
                    expected: type_id,
                }));
            }
        };
        json_string.encode_to(out);
        Ok(())
    }
}

#[cfg(feature = "parity-encoding")]
impl parity_scale_codec::Encode for SerdeWrapper {
    fn encode_to<T: parity_scale_codec::Output + ?Sized>(&self, dest: &mut T) {
        let json_string =
            serde_json::to_string(&self.0).expect("Failed to serialize Value to JSON string");
        json_string.encode_to(dest);
    }
}

#[cfg(feature = "parity-encoding")]
impl parity_scale_codec::Decode for SerdeWrapper {
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

#[cfg(feature = "parity-encoding")]
impl scale_info::TypeInfo for SerdeWrapper {
    type Identity = Self;

    fn type_info() -> scale_info::Type {
        scale_info::Type::builder()
            .path(scale_info::Path::new("SerdeWrapper", module_path!()))
            .composite(scale_info::build::Fields::unnamed().field(|f| f.ty::<String>()))
    }
}

impl From<SerdeWrapper> for Value {
    fn from(wrapper: SerdeWrapper) -> Self {
        wrapper.0
    }
}

#[cfg_attr(
    feature = "parity-encoding",
    derive(
        scale_info::TypeInfo,
        parity_scale_codec::Encode,
        parity_scale_codec::Decode,
        scale_encode::EncodeAsType
    )
)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Attribute {
    pub typ: String,
    pub value: SerdeWrapper,
}

impl core::fmt::Display for Attribute {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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
        Self { typ: typ.as_ref().to_owned(), value: value.into() }
    }
}

#[cfg_attr(
    feature = "parity-encoding",
    derive(
        scale_encode::EncodeAsType,
        scale_info::TypeInfo,
        parity_scale_codec::Encode,
        parity_scale_codec::Decode,
    )
)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Attributes {
    typ: Option<DomaintypeId>,
    items: Vec<Attribute>,
}

impl Attributes {
    pub fn new(typ: Option<DomaintypeId>, mut items: Vec<Attribute>) -> Self {
        let mut seen_types = BTreeSet::new();
        items.retain(|attr| seen_types.insert(attr.typ.clone()));
        items.sort_by(|a, b| a.typ.cmp(&b.typ));
        Self { typ, items }
    }

    pub fn get_attribute(&self, key: &str) -> Option<&Attribute> {
        self.items.iter().find(|&attribute| attribute.typ == key)
    }

    #[tracing::instrument(skip(self))]
    pub fn get_values(&self) -> Vec<&Attribute> {
        self.items.iter().collect()
    }

    pub fn type_only(typ: Option<DomaintypeId>) -> Self {
        Self { typ, items: Vec::new() }
    }

    pub fn get_typ(&self) -> &Option<DomaintypeId> {
        &self.typ
    }

    pub fn get_items(&self) -> &[Attribute] {
        &self.items
    }

    pub fn into_items(self) -> Vec<Attribute> {
        self.items
    }

    pub fn add_item(&mut self, value: Attribute) {
        if !self.items.iter().any(|item| item.typ == value.typ) {
            if let Some(pos) = self.items.iter().position(|item| item.typ > value.typ) {
                self.items.insert(pos, value);
            } else {
                self.items.push(value);
            }
        }
    }
}
