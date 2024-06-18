use diesel::{
	pg::Pg,
	prelude::*,
	query_builder::*,
	r2d2::{ConnectionManager, PooledConnection},
	sql_types::BigInt,
};

type Conn = PooledConnection<ConnectionManager<PgConnection>>;

const DEFAULT_PAGE_SIZE: i32 = 10;

#[derive(QueryId)]
pub struct CursorPosition<T> {
	query: T,
	pub start: i64,
	pub limit: i64,
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
