use async_graphql::{Object, SimpleObject};
use chrono::NaiveDateTime;
use diesel::{Queryable, Selectable};

#[derive(Default, Queryable, Selectable, SimpleObject)]
#[diesel(table_name = crate::schema::agent)]
pub struct Agent {
	pub id: i32,
	pub external_id: String,
	pub namespace_id: i32,
	pub domaintype: Option<String>,
	pub current: i32,
	pub identity_id: Option<i32>,
}

#[derive(Default, Queryable, Selectable, SimpleObject)]
#[diesel(table_name = crate::schema::activity)]
pub struct Activity {
	pub id: i32,
	pub external_id: String,
	pub namespace_id: i32,
	pub domaintype: Option<String>,
	pub started: Option<NaiveDateTime>,
	pub ended: Option<NaiveDateTime>,
}

#[derive(Queryable, Selectable, SimpleObject)]
#[diesel(table_name = crate::schema::entity)]
pub struct Entity {
	pub id: i32,
	pub external_id: String,
	pub namespace_id: i32,
	pub domaintype: Option<String>,
}

#[derive(Default, Queryable)]
pub struct Namespace {
	_id: i32,
	uuid: String,
	external_id: String,
}

#[Object]
/// # `chronicle:Namespace`
///
/// An IRI containing an external id and uuid part, used for disambiguation.
/// In order to work on the same namespace discrete Chronicle instances must share
/// the uuid part.
impl Namespace {
	async fn external_id(&self) -> &str {
		&self.external_id
	}

	async fn uuid(&self) -> &str {
		&self.uuid
	}
}
