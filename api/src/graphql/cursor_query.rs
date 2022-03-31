use diesel::prelude::*;
use diesel::query_builder::*;
use diesel::r2d2::ConnectionManager;
use diesel::sql_types::BigInt;
use diesel::sqlite::Sqlite;
use r2d2::PooledConnection;

type Conn = PooledConnection<ConnectionManager<SqliteConnection>>;

const DEFAULT_PAGE_SIZE: i32 = 10;

#[derive(QueryId)]
pub struct CursorPosition<T> {
    query: T,
    start: i64,
    limit: i64,
}

pub trait Paginate: Sized {
    fn cursor(
        self,
        after: Option<i32>,
        before: Option<i32>,
        first: Option<i32>,
        last: Option<i32>,
    ) -> CursorPosition<Self>;
}

impl<T> Paginate for T {
    fn cursor(
        self,
        after: Option<i32>,
        before: Option<i32>,
        first: Option<i32>,
        last: Option<i32>,
    ) -> CursorPosition<Self> {
        let mut start = after.map(|after| after + 1).unwrap_or(0);
        let mut end = before.unwrap_or(DEFAULT_PAGE_SIZE);
        if let Some(first) = first {
            end = (start + first).min(end);
        }
        if let Some(last) = last {
            start = if last > end - start { end } else { end - last };
        };

        CursorPosition {
            query: self,
            start: start as _,
            limit: (end - start) as _,
        }
    }
}

impl<T> QueryFragment<Sqlite> for CursorPosition<T>
where
    T: QueryFragment<Sqlite>,
{
    fn walk_ast<'a, 'b>(&'b self, mut out: AstPass<'a, 'b, Sqlite>) -> QueryResult<()> {
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
