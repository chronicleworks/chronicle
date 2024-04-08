use clap::{Arg, Command};
fn in_subcommand() -> Command<'static> {
	Command::new("in").about("Handles incoming data operations")
}

fn out_subcommand() -> Command<'static> {
	Command::new("out").about("Handles outgoing data operations")
}

fn describe_subcommand() -> Command<'static> {
	Command::new("describe")
		.about("Describes the data schema and operations")
		.subcommand(Command::new("schema").about("Describes the data schema"))
		.subcommand(Command::new("flights").about("List the available flights"))
}

pub fn build_cli() -> Command<'static> {
	Command::new("chronicle-data")
		.about("CLI for Chronicle Data operations")
		.arg(
			Arg::new("chronicle")
				.long("chronicle")
				.help("The Chronicle server URL")
				.takes_value(true)
				.global(true)
				.required(true),
		)
		.arg(
			Arg::new("auth")
				.long("auth")
				.help("Authentication token")
				.takes_value(true)
				.global(true)
				.required(false),
		)
		.subcommand(describe_subcommand())
		.subcommand(in_subcommand())
		.subcommand(out_subcommand())
}
