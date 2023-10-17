use chrono::{DateTime, NaiveDateTime, Utc};
use scale_info::{build::Variants, Path, Type, TypeInfo, Variant};

use crate::{
    attributes::Attribute,
    prov::{ChronicleIri, NamespaceId},
};

use parity_scale_codec::{Decode, Encode};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct Contradiction {
    pub(crate) id: ChronicleIri,
    pub(crate) namespace: NamespaceId,
    pub(crate) contradiction: Vec<ContradictionDetail>,
}

impl std::error::Error for Contradiction {
    fn source(&self) -> Option<&(dyn custom_error::Error + 'static)> {
        None
    }

    fn cause(&self) -> Option<&dyn custom_error::Error> {
        self.source()
    }
}

impl std::fmt::Display for Contradiction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Contradiction {{ ")?;
        for detail in &self.contradiction {
            match detail {
                ContradictionDetail::AttributeValueChange {
                    name,
                    value,
                    attempted,
                } => {
                    write!(f, "attribute value change: {name} {value:?} {attempted:?}")?;
                }
                ContradictionDetail::StartAlteration { value, attempted } => {
                    write!(f, "start date alteration: {value} {attempted}")?;
                }
                ContradictionDetail::EndAlteration { value, attempted } => {
                    write!(f, "end date alteration: {value} {attempted}")?;
                }
                ContradictionDetail::InvalidRange { start, end } => {
                    write!(f, "invalid range: {start} {end}")?;
                }
            }
        }
        write!(f, " }}")
    }
}

impl Contradiction {
    pub fn start_date_alteration(
        id: ChronicleIri,
        namespace: NamespaceId,
        value: DateTime<Utc>,
        attempted: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            namespace,
            contradiction: vec![ContradictionDetail::StartAlteration { value, attempted }],
        }
    }

    pub fn end_date_alteration(
        id: ChronicleIri,
        namespace: NamespaceId,
        value: DateTime<Utc>,
        attempted: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            namespace,
            contradiction: vec![ContradictionDetail::EndAlteration { value, attempted }],
        }
    }

    pub fn invalid_range(
        id: ChronicleIri,
        namespace: NamespaceId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            namespace,
            contradiction: vec![ContradictionDetail::InvalidRange { start, end }],
        }
    }

    pub fn attribute_value_change(
        id: ChronicleIri,
        namespace: NamespaceId,
        changes: Vec<(String, Attribute, Attribute)>,
    ) -> Self {
        Self {
            id,
            namespace,
            contradiction: changes
                .into_iter()
                .map(
                    |(name, value, attempted)| ContradictionDetail::AttributeValueChange {
                        name,
                        value,
                        attempted,
                    },
                )
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum ContradictionDetail {
    AttributeValueChange {
        name: String,
        value: Attribute,
        attempted: Attribute,
    },
    StartAlteration {
        value: DateTime<Utc>,
        attempted: DateTime<Utc>,
    },
    EndAlteration {
        value: DateTime<Utc>,
        attempted: DateTime<Utc>,
    },
    InvalidRange {
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    },
}

impl TypeInfo for ContradictionDetail {
    type Identity = Self;

    fn type_info() -> Type {
        Type::builder()
            .path(Path::new("ContradictionDetail", "prov"))
            .variant(
                Variants::new()
                    .variant("AttributeValueChange", |v| v.index(0))
                    .variant("StartAlteration", |v| v.index(1))
                    .variant("EndAlteration", |v| v.index(2))
                    .variant("InvalidRange", |v| v.index(3)),
            )
    }
}

impl Encode for ContradictionDetail {
    fn encode_to<T: ?Sized + parity_scale_codec::Output>(&self, dest: &mut T) {
        match self {
            ContradictionDetail::AttributeValueChange {
                name,
                value,
                attempted,
            } => {
                dest.push_byte(0);
                name.encode_to(dest);
                value.encode_to(dest);
                attempted.encode_to(dest);
            }
            ContradictionDetail::StartAlteration { value, attempted } => {
                dest.push_byte(1);
                value.timestamp().encode_to(dest);
                attempted.timestamp().encode_to(dest);
            }
            ContradictionDetail::EndAlteration { value, attempted } => {
                dest.push_byte(2);
                value.timestamp().encode_to(dest);
                attempted.timestamp().encode_to(dest);
            }
            ContradictionDetail::InvalidRange { start, end } => {
                dest.push_byte(3);
                start.timestamp().encode_to(dest);
                end.timestamp().encode_to(dest);
            }
        }
    }
}

impl Decode for ContradictionDetail {
    fn decode<I: Sized + parity_scale_codec::Input>(
        input: &mut I,
    ) -> Result<Self, parity_scale_codec::Error> {
        match input.read_byte()? {
            0 => Ok(ContradictionDetail::AttributeValueChange {
                name: Decode::decode(input)?,
                value: Decode::decode(input)?,
                attempted: Decode::decode(input)?,
            }),
            1 => Ok(ContradictionDetail::StartAlteration {
                value: DateTime::<Utc>::from_utc(
                    NaiveDateTime::from_timestamp(Decode::decode(input)?, 0),
                    Utc,
                ),
                attempted: DateTime::<Utc>::from_utc(
                    NaiveDateTime::from_timestamp(Decode::decode(input)?, 0),
                    Utc,
                ),
            }),
            2 => Ok(ContradictionDetail::EndAlteration {
                value: DateTime::<Utc>::from_utc(
                    NaiveDateTime::from_timestamp(Decode::decode(input)?, 0),
                    Utc,
                ),
                attempted: DateTime::<Utc>::from_utc(
                    NaiveDateTime::from_timestamp(Decode::decode(input)?, 0),
                    Utc,
                ),
            }),
            3 => Ok(ContradictionDetail::InvalidRange {
                start: DateTime::<Utc>::from_utc(
                    NaiveDateTime::from_timestamp(Decode::decode(input)?, 0),
                    Utc,
                ),
                end: DateTime::<Utc>::from_utc(
                    NaiveDateTime::from_timestamp(Decode::decode(input)?, 0),
                    Utc,
                ),
            }),
            _ => Err(parity_scale_codec::Error::from(
                "Unknown variant identifier",
            )),
        }
    }
}
