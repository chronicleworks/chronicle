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
    pub(crate) start: i64,
    pub(crate) limit: i64,
}

macro_rules! gql_cursor {
    ($after:expr, $before: expr, $first: expr, $last: expr, $query:expr, $order:expr, $node_type:tt,$connection: expr) => {{
        use crate::chronicle_graphql::cursor_query::Cursorise;
        use crate::chronicle_graphql::GraphQlError;
        use async_graphql::connection::query;
        use async_graphql::connection::{Connection, Edge, EmptyFields};

        query(
            $after,
            $before,
            $first,
            $last,
            |after, before, first, last| async move {
                let rx = $query
                    .order($order)
                    .select(<$node_type>::as_select())
                    .cursor(after, before, first, last);

                let start = rx.start;
                let limit = rx.limit;

                let rx = rx.load::<($node_type, i64)>(&mut $connection)?;

                let mut gql = Connection::new(
                    rx.first().map(|(_, _total)| start > 0).unwrap_or(false),
                    rx.first()
                        .map(|(_, total)| ((start as i64) + (limit as i64)) < *total)
                        .unwrap_or(false),
                );

                gql.append(rx.into_iter().enumerate().map(
                    (|(pos, (agent, _count))| {
                        Edge::with_additional_fields(
                            (pos as i32) + (start as i32),
                            agent,
                            EmptyFields,
                        )
                    }),
                ));

                Ok::<_, GraphQlError>(gql)
            },
        )
        .await
    }};
}

pub trait Cursorise: Sized {
    fn cursor(
        self,
        after: Option<i32>,
        before: Option<i32>,
        first: Option<usize>,
        last: Option<usize>,
    ) -> CursorPosition<Self>;
}

impl<T> Cursorise for T {
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
