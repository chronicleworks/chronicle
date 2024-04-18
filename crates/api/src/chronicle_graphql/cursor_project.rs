use async_graphql::{
	connection::{Edge, EmptyFields},
	OutputType,
};

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
