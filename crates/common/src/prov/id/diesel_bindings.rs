use super::*;
use diesel::{
	backend::Backend,
	deserialize::FromSql,
	serialize::{Output, ToSql},
	sql_types::Text,
};

impl<DB> ToSql<Text, DB> for Role
where
	DB: Backend,
	String: ToSql<Text, DB>,
{
	fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, DB>) -> diesel::serialize::Result {
		self.0.to_sql(out)
	}
}

impl<DB> FromSql<Text, DB> for Role
where
	DB: Backend,
	String: FromSql<Text, DB>,
{
	fn from_sql(bytes: <DB as Backend>::RawValue<'_>) -> diesel::deserialize::Result<Self> {
		Ok(Self(String::from_sql(bytes)?))
	}
}

impl<DB> ToSql<Text, DB> for ExternalId
where
	DB: Backend,
	String: ToSql<Text, DB>,
{
	fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, DB>) -> diesel::serialize::Result {
		self.0.to_sql(out)
	}
}

impl<DB> FromSql<Text, DB> for ExternalId
where
	DB: Backend,
	String: FromSql<Text, DB>,
{
	fn from_sql(bytes: <DB as Backend>::RawValue<'_>) -> diesel::deserialize::Result<Self> {
		Ok(Self(String::from_sql(bytes)?))
	}
}
