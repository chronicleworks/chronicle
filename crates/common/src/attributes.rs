use std::collections::BTreeMap;
use parity_scale_codec:: {Encode,Decode};
use scale_info::{TypeInfo, Type, Path, build::Fields};
use serde_json::Value;

use crate::prov::DomaintypeId;

#[derive(Debug, Clone,  Serialize, Deserialize, PartialEq, Eq)]
pub struct Attribute {
    pub typ: String,
    pub value: Value,
}


impl TypeInfo for Attribute {
    type Identity = Self;

    fn type_info() -> Type {
        Type::builder()
            .path(Path::new("Attribute", module_path!()))
            .composite(
                Fields::named()
                    .field(|f| f.ty::<String>().name("Type").type_name("String"))
                    .field(|f| f.ty::<String>().name("Value").type_name("String"))
            )
    }
}






impl Encode for Attribute {
    fn encode_to<T: parity_scale_codec::Output + ?Sized>(&self, dest: &mut T) {
        self.typ.encode_to(dest);
        self.value.to_string().encode_to(dest);
    }

    fn size_hint(&self) -> usize {
        self.typ.size_hint() + self.value.to_string().size_hint()
    }
}

impl Decode for Attribute {
    fn decode<I: parity_scale_codec::Input>(input: &mut I) -> Result<Self, parity_scale_codec::Error> {
        let typ = String::decode(input)?;
        let value_str = String::decode(input)?;
        let value = serde_json::from_str(&value_str).map_err(|_| parity_scale_codec::Error::from("Failed to decode Value"))?;
        Ok(Self { typ, value })
    }
}


impl Attribute {
    pub fn new(typ: impl AsRef<str>, value: Value) -> Self {
        Self {
            typ: typ.as_ref().to_owned(),
            value,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode, TypeInfo, PartialEq, Eq, Default)]
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
