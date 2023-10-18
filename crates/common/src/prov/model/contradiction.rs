use chrono::{DateTime, NaiveDateTime, Utc};
use scale_info::{build::Variants, Path, Type, TypeInfo, Variant};

use crate::{
    attributes::Attribute,
    prov::{operations::TimeWrapper, ChronicleIri, NamespaceId},
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
            contradiction: vec![ContradictionDetail::StartAlteration {
                value: value.into(),
                attempted: attempted.into(),
            }],
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
            contradiction: vec![ContradictionDetail::EndAlteration {
                value: value.into(),
                attempted: attempted.into(),
            }],
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
            contradiction: vec![ContradictionDetail::InvalidRange {
                start: start.into(),
                end: end.into(),
            }],
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

#[derive(Debug, Clone, Encode, Decode, TypeInfo, Serialize, Deserialize, Eq, PartialEq)]
pub enum ContradictionDetail {
    AttributeValueChange {
        name: String,
        value: Attribute,
        attempted: Attribute,
    },
    StartAlteration {
        value: TimeWrapper,
        attempted: TimeWrapper,
    },
    EndAlteration {
        value: TimeWrapper,
        attempted: TimeWrapper,
    },
    InvalidRange {
        start: TimeWrapper,
        end: TimeWrapper,
    },
}
