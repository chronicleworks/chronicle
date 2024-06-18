use arrow_flight::{flight_service_client::FlightServiceClient, FlightInfo};
use arrow_schema::Schema;
use clap::Parser;
use cli::{Cli, Commands, DescribeSubcommands};
use prettytable::{format, row, Cell, Row, Table};
use tonic::transport::Channel;

mod cli;

async fn init_flight_client(
	cli: &Cli,
) -> Result<FlightServiceClient<Channel>, Box<dyn std::error::Error>> {
	let chronicle_url = &cli.chronicle;
	let channel = Channel::from_shared(chronicle_url.clone())?.connect().await?;

	Ok(FlightServiceClient::new(channel))
}

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

fn format_flight_info_as_table(_flight_infos: Vec<FlightInfo>) -> String {
	let mut table = Table::new();
	table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);

	table.set_titles(row!["Descriptor", "Endpoints", "Summary"]);

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
	let cli = Cli::parse();
	let _client = init_flight_client(&cli).await.expect("Failed to initialize the Flight client");

	match &cli.command {
		Commands::Describe { subcommand } => match subcommand {
			DescribeSubcommands::Schema => {
				println!("Describing the data schema...");
			},
			DescribeSubcommands::Flights => {
				println!("Listing available flights...");
			},
		},
		Commands::In => {
			println!("Handling incoming data operations...");
		},
		Commands::Out => {
			println!("Handling outgoing data operations...");
		},
	}
}
