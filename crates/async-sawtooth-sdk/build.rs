use std::io::Result;

fn main() -> Result<()> {
    let protos = glob::glob("./src/protos/*.proto")
        .unwrap()
        .into_iter()
        .map(|x| x.unwrap())
        .collect::<Vec<_>>();
    prost_build::compile_protos(&protos, &["./src/protos"])?;
    Ok(())
}
