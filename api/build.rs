fn main() -> Result<(), std::io::Error> {
    use btl::{cd, shell};
    use std::{env, path::PathBuf};

    //let release = env::var("PROFILE").expect("expected PROFILE to be set by Cargo") != "debug";
    //let out_dir = PathBuf::from(env::var("OUT_DIR").expect("expected OUT_DIR to be set by Cargo"));

    let files_dir: std::path::PathBuf = [PathBuf::from("../web-target")].iter().collect();
    bui_backend_codegen::codegen(&files_dir, "public.rs").expect("codegen failed");

    Ok(())
}
