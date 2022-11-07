use cli::load_key_from_match;

mod cli;

fn main() {
    let matches = cli::cli().get_matches();

    match matches.subcommand() {
        Some(("bootstrap", matches)) => {
            let _root_key = load_key_from_match("root-key", matches);
        }
        Some(("generate", _matches)) => {}
        Some(("rotate-root", _matches)) => {}
        Some(("register-key", _matches)) => {}
        Some(("rotate-key", _matches)) => {}
        Some(("set-policy", _matches)) => {}
        Some(("get-key", _matches)) => {}
        Some(("get-policy", _matches)) => {}
        _ => unreachable!(),
    }
}
