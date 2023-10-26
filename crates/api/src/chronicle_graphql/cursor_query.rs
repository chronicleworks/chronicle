use async_graphql::{
	connection::{Edge, EmptyFields},
	OutputType,
};
use diesel::{pg::Pg, prelude::*, query_builder::*, r2d2::ConnectionManager, sql_types::BigInt};
use r2d2::PooledConnection;

type Conn = PooledConnection<ConnectionManager<PgConnection>>;

const DEFAULT_PAGE_SIZE: i32 = 10;

#[derive(QueryId)]
pub struct CursorPosition<T> {
	query: T,
	pub(crate) start: i64,
	pub(crate) limit: i64,
}
pub trait Cursorize: Sized {
	fn cursor(
		self,
		after: Option<i32>,
		before: Option<i32>,
		first: Option<usize>,
		last: Option<usize>,
	) -> CursorPosition<Self>;
}

pub fn project_to_nodes<T, I>(
	rx: I,
	start: i64,
	limit: i64,
) -> async_graphql::connection::Connection<i32, T, EmptyFields, EmptyFields>
where
	T: OutputType,
	I: IntoIterator<Item = (T, i64)>,
{
	let rx = Vec::from_iter(rx);
	let mut gql = async_graphql::connection::Connection::new(
		rx.first().map(|(_, _total)| start > 0).unwrap_or(false),
		rx.first().map(|(_, total)| start + limit < *total).unwrap_or(false),
	);

	gql.edges.append(
		&mut rx
			.into_iter()
			.enumerate()
			.map(|(pos, (agent, _count))| {
				Edge::with_additional_fields((pos as i32) + (start as i32), agent, EmptyFields)
			})
			.collect(),
	);
	gql
}

impl<T> Cursorize for T {
	fn cursor(
		self,
		after: Option<i32>,
		before: Option<i32>,
		first: Option<usize>,
		last: Option<usize>,
	) -> CursorPosition<Self> {
		let mut start = after.map(|after| after + 1).unwrap_or(0) as usize;
		let mut end = before.unwrap_or(DEFAULT_PAGE_SIZE) as usize;
		if let Some(first) = first {
			end = start + first
		}
		if let Some(last) = last {
			start = if last > end - start { end } else { end - last };
		};

		CursorPosition { query: self, start: start as _, limit: (end - start) as _ }
	}
}

impl<T> QueryFragment<Pg> for CursorPosition<T>
where
	T: QueryFragment<Pg>,
{
	fn walk_ast<'a>(&'a self, mut out: AstPass<'_, 'a, Pg>) -> QueryResult<()> {
		out.push_sql("SELECT *, COUNT(*) OVER () FROM (");
		self.query.walk_ast(out.reborrow())?;
		out.push_sql(") t LIMIT ");
		out.push_bind_param::<BigInt, _>(&(self.limit))?;
		out.push_sql(" OFFSET ");
		out.push_bind_param::<BigInt, _>(&self.start)?;
		Ok(())
	}
}

impl<T: Query> Query for CursorPosition<T> {
	type SqlType = (T::SqlType, BigInt);
}

impl<T> RunQueryDsl<Conn> for CursorPosition<T> {}
