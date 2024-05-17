use clap::{Parser, Subcommand};

#[derive(Parser)]
#[clap(name = "chronicle-data", about = "CLI for Chronicle Data operations")]
pub struct Cli {
    #[arg(long, help = "The Chronicle server URL", global = true, required = true)]
    pub chronicle: String,

    #[arg(long, help = "Authentication token", global = true, required = false)]
    pub auth: Option<String>,

    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    #[clap(about = "Describes the data schema and operations")]
    Describe {
        #[clap(subcommand)]
        subcommand: DescribeSubcommands,
    },
    #[clap(about = "Handles incoming data operations")]
    In,
    #[clap(about = "Handles outgoing data operations")]
    Out,
}

#[derive(Subcommand)]
pub enum DescribeSubcommands {
    #[clap(about = "Describes the data schema")]
    Schema,
    #[clap(about = "List the available flights")]
    Flights,
}
