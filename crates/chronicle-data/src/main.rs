mod cli;

use arrow_flight::flight_service_client::FlightServiceClient;
use tonic::transport::Channel;

async fn init_flight_client(
	matches: &clap::ArgMatches,
) -> Result<FlightServiceClient<Channel>, Box<dyn std::error::Error>> {
	let chronicle_url = matches
		.value_of("chronicle")
		.expect("Chronicle server URL is required")
		.to_string();
	let channel = Channel::from_shared(chronicle_url)?.connect().await?;

	Ok(FlightServiceClient::new(channel))
}

use arrow_schema::Schema;
use prettytable::{format, row, Cell, Row, Table};

fn format_schema_as_table(schema: &Schema) -> String {
	let mut table = Table::new();
	table.add_row(row!["Field Name", "Data Type", "Nullable"]);
	for field in schema.fields() {
		table.add_row(Row::new(vec![
			Cell::new(field.name()),
			Cell::new(&format!("{:?}", field.data_type())),
			Cell::new(&format!("{}", field.is_nullable())),
		]));
	}
	table.to_string()
}

use arrow_flight::FlightInfo;

fn format_flight_info_as_table(flight_infos: Vec<FlightInfo>) -> String {
	let mut table = Table::new();
	table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);

	table.set_titles(row!["Descriptor", "Endpoints", "Summary"]);

	let grouped_by_descriptor: std::collections::HashMap<String, Vec<FlightInfo>> =
		std::collections::HashMap::new();

	table.to_string()
}

async fn list_flights(
	client: &mut FlightServiceClient<Channel>,
) -> Result<Vec<arrow_flight::FlightInfo>, Box<dyn std::error::Error>> {
	let request = tonic::Request::new(arrow_flight::Criteria::default());
	let response = client.list_flights(request).await?;

	let mut flights_info = Vec::new();
	let mut stream = response.into_inner();
	while let Some(flight_info) = stream.message().await? {
		flights_info.push(flight_info);
	}

	Ok(flights_info)
}

#[tokio::main]
async fn main() {
	let matches = cli::build_cli().get_matches();
	let client = init_flight_client(&matches)
		.await
		.expect("Failed to initialize the Flight client");

	if let Some(subcommand) = matches.subcommand() {
		match subcommand {
			("describe", sub_matches) =>
				if sub_matches.subcommand_matches("schema").is_some() {
					println!("Describing the data schema...");
				} else if sub_matches.subcommand_matches("flights").is_some() {
					println!("Listing available flights...");
				},
			("in", _) => {
				println!("Handling incoming data operations...");
			},
			("out", _) => {
				println!("Handling outgoing data operations...");
			},
			_ => unreachable!(),
		}
	}
}
