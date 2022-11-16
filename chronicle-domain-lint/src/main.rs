use chronicle::codegen::linter::check_files;
use clap::{Arg, Command, ValueHint};

fn main() {
    let cli = Command::new("chronicle-domain-lint")
        .version("0.1")
        .author("Blockchain Technology Partners")
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
