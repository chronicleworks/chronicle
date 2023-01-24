use chrono::{DateTime, Utc};

use crate::{
    attributes::Attribute,
    prov::{ChronicleIri, NamespaceId},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
