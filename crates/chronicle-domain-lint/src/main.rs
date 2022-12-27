use chronicle::codegen::linter::check_files;
use clap::{Arg, Command, ValueHint};

fn main() {
    let version = env!("CARGO_PKG_VERSION");
    let cli = Command::new("chronicle-domain-lint")
        .version(version)
        .author("BTPWorks")
        .arg(
            Arg::new("filenames")
                .value_hint(ValueHint::FilePath)
                .required(true)
                .multiple_values(true)
                .min_values(1)
                .help("domain definition files for linting"),
        );

    let matches = cli.get_matches();
    let filenames = matches.values_of("filenames").unwrap().collect();
    check_files(filenames);
    println!("successful: no domain definition errors detected");
}
