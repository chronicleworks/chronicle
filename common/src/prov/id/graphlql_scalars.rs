use async_graphql::{InputValueError, InputValueResult, Scalar, ScalarType, Value};
use iref::Iri;

use super::{ActivityId, AgentId, DomaintypeId, EntityId, EvidenceId, IdentityId};

#[Scalar(name = "AttachmentID")]
impl ScalarType for EvidenceId {
    fn parse(value: Value) -> InputValueResult<Self> {
        if let Value::String(value) = value {
            // Parse the integer value
            Ok(EvidenceId::try_from(Iri::from_str(&*value)?)?)
        } else {
            // If the type does not match
            Err(InputValueError::expected_type(value))
        }
    }

    fn to_value(&self) -> Value {
        Value::String(self.to_string())
    }
}

#[Scalar(name = "IdentityID")]
impl ScalarType for IdentityId {
    fn parse(value: Value) -> InputValueResult<Self> {
        if let Value::String(value) = &value {
            // Parse the integer value
            Ok(IdentityId::try_from(Iri::from_str(&*value)?)?)
        } else {
            // If the type does not match
            Err(InputValueError::expected_type(value))
        }
    }

    fn to_value(&self) -> Value {
        Value::String(self.to_string())
    }
}

#[Scalar(name = "DomaintypeID")]
impl ScalarType for DomaintypeId {
    fn parse(value: Value) -> InputValueResult<Self> {
        if let Value::String(value) = &value {
            // Parse the integer value
            Ok(DomaintypeId::try_from(Iri::from_str(&*value)?)?)
        } else {
            // If the type does not match
            Err(InputValueError::expected_type(value))
        }
    }

    fn to_value(&self) -> Value {
        Value::String(self.to_string())
    }
}

#[Scalar(name = "EntityID")]
impl ScalarType for EntityId {
    fn parse(value: Value) -> InputValueResult<Self> {
        if let Value::String(value) = &value {
            // Parse the integer value
            Ok(EntityId::try_from(Iri::from_str(&*value)?)?)
        } else {
            // If the type does not match
            Err(InputValueError::expected_type(value))
        }
    }

    fn to_value(&self) -> Value {
        Value::String(self.to_string())
    }
}

#[Scalar(name = "AgentID")]
impl ScalarType for AgentId {
    fn parse(value: Value) -> InputValueResult<Self> {
        if let Value::String(value) = &value {
            // Parse the integer value
            Ok(AgentId::try_from(Iri::from_str(&*value)?)?)
        } else {
            // If the type does not match
            Err(InputValueError::expected_type(value))
        }
    }

    fn to_value(&self) -> Value {
        Value::String(self.to_string())
    }
}

#[Scalar(name = "ActivityID")]
impl ScalarType for ActivityId {
    fn parse(value: Value) -> InputValueResult<Self> {
        if let Value::String(value) = &value {
            // Parse the integer value
            Ok(ActivityId::try_from(Iri::from_str(&*value)?)?)
        } else {
            // If the type does not match
            Err(InputValueError::expected_type(value))
        }
    }

    fn to_value(&self) -> Value {
        Value::String(self.to_string())
    }
}
