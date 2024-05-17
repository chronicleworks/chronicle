use async_graphql::{InputValueError, InputValueResult, Scalar, ScalarType, Value};

use super::{ActivityId, AgentId, ChronicleJSON, DomaintypeId, EntityId};

async_graphql::scalar!(ChronicleJSON);

#[Scalar(name = "DomaintypeID")]
/// Derived from an `Activity`'s or `Agent`'s or `Entity`'s subtype.
/// The built-in GraphQL field `__TypeName` should be used for union queries.
impl ScalarType for DomaintypeId {
    fn parse(value: Value) -> InputValueResult<Self> {
        if let Value::String(value) = value {
            // Parse the integer value
            Ok(DomaintypeId::try_from(value)?)
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
/// This is derived from an `Entity`'s externalId, but clients
/// should not attempt to synthesize it themselves.
impl ScalarType for EntityId {
    fn parse(value: Value) -> InputValueResult<Self> {
        if let Value::String(value) = value {
            // Parse the integer value
            Ok(EntityId::try_from(value)?)
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
/// This is derived from an `Agent`'s externalId, but clients
/// should not attempt to synthesize it themselves.
impl ScalarType for AgentId {
    fn parse(value: Value) -> InputValueResult<Self> {
        if let Value::String(value) = value {
            // Parse the integer value
            Ok(AgentId::try_from(value)?)
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
/// This is derived from an `Activity`'s externalId, but clients
/// should not attempt to synthesize it themselves.
impl ScalarType for ActivityId {
    fn parse(value: Value) -> InputValueResult<Self> {
        if let Value::String(value) = value {
            // Parse the integer value
            Ok(ActivityId::try_from(value)?)
        } else {
            // If the type does not match
            Err(InputValueError::expected_type(value))
        }
    }

    fn to_value(&self) -> Value {
        Value::String(self.to_string())
    }
}
