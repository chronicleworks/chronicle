use std::{env, fs, io::Result, path::PathBuf};

include!("./src/context.rs");

fn main() -> Result<()> {
    let protos = glob::glob("./src/protos/*.proto")
        .unwrap()
        .into_iter()
        .map(|x| x.unwrap())
        .collect::<Vec<_>>();
    prost_build::compile_protos(&protos, &["./src/protos"])?;

    let out_str = env::var("OUT_DIR").unwrap();
    let out_path = PathBuf::from(&out_str);
    let mut out_path = out_path.ancestors().nth(3).unwrap().to_owned();
    out_path.push("assets");

    if !out_path.exists() {
        fs::create_dir(&out_path).expect("Could not create assets dir");
    }

    let context = &PROV;

    std::fs::write(
        std::path::Path::new(&format!(
            "{}/context.json",
            out_path.as_os_str().to_string_lossy(),
        )),
        context.pretty(2),
    )?;

    let policies = vec![
        ("allow_defines", "allow"),
        ("auth", "is_authenticated"),
        ("default_allow", "allow"),
        ("default_deny", "allow"),
    ];

    for (policy_name, entrypoint) in policies {
        opa::build::policy(policy_name)
            .add_source(format!("src/policies/{policy_name}.rego"))
            .add_entrypoint(format!("{policy_name}.{entrypoint}"))
            .compile()
            .unwrap();
    }

    Ok(())
}
