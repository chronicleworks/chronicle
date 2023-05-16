//! A command-line interface for generating Chronicle Synth schema for a given domain.

use std::{
    fs::File,
    io::{BufReader, Write},
    path::{Path, PathBuf},
};

use chronicle::codegen::linter::check_files;
use chronicle_synth::{
    collection::{Collection, CollectionHandling},
    domain::TypesAttributesRoles,
    error::ChronicleSynthError,
};
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(
    name = "chronicle-domain-synth",
    about = "Generate Chronicle Synth schema for your domain",
    author = "Blockchain Technology Partners"
)]
struct Cli {
    #[structopt(
        value_name = "FILE",
        help = "Chronicle domain definition file",
        parse(from_os_str),
        default_value = "crates/chronicle-synth/domain.yaml"
    )]
    domain_file: PathBuf,
}

const COLLECT_SCRIPT: &str = "./crates/chronicle-synth/collect";

fn main() -> Result<(), ChronicleSynthError> {
    let args = Cli::from_args();

    let domain_file = args.domain_file.as_path();

    // Use Chronicle Domain Linter to check the domain definition file
    let filenames = vec![domain_file.to_str().ok_or_else(|| {
        ChronicleSynthError::IO(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid path argument",
        ))
    })?];

    check_files(filenames);

    println!("{}", "No domain definition errors detected.".green());

    generate_synth_collections(domain_file)?;

    // Run the `collect` script to collate the complete set of Synth collections for the domain
    let output = std::process::Command::new("bash")
        .args([COLLECT_SCRIPT])
        .output()
        .expect("Failed to execute 'collect' command");

    println!("{}", String::from_utf8_lossy(&output.stdout));

    println!(
        "{} contains the additional Synth collections generated for your domain.",
        "crates/chronicle-synth/domain-schema/".underline()
    );
    println!(
        "The complete set of Synth collections for your domain can be found in '{}'.",
        "crates/chronicle-synth/collections/".underline()
    );

    Ok(())
}

/// Generates Synth collections for the given domain definition file.
///
/// This function takes a path to a domain definition file, generates Synth collections based on the
/// definition, and writes the resulting schema files to the `domain-schema` directory. Collections marked
/// as not being "chronicle operations" are written to a file called `exclude_collections.txt`.
///
/// # Arguments
///
/// * `domain_file` - A path to the domain definition file.
///
/// # Examples
///
/// ```
/// use std::path::PathBuf;
/// use chronicle_domain_synth::ChronicleSynthError;
///
/// let domain_file = PathBuf::from("domain.yaml");
/// let result = generate_synth_collections(&domain_file);
///
/// match result {
///     Ok(_) => println!("Synth collections generated successfully."),
///     Err(e) => eprintln!("Error generating Synth collections: {}", e),
/// }
/// ```
fn generate_synth_collections(domain_file: &Path) -> Result<(), ChronicleSynthError> {
    let generator = TypesAttributesRoles::from_file(domain_file)?;
    println!(
        "Generating schema for domain: {}.",
        generator.name.underline()
    );

    let dir_path = PathBuf::from(DOMAIN_SCHEMA_TARGET_DIRECTORY);
    std::fs::create_dir_all(&dir_path)?;

    let collections = generator.generate_domain_collections()?;
    for collection in collections {
        write_collection(&collection, &dir_path)?;

        match collection {
            Collection::Operation(_) => {}
            Collection::Generator(collection) => {
                append_to_exclude_list(EXCLUDE_LIST, &collection.name())?;
            }
        }
    }
    Ok(())
}

const DOMAIN_SCHEMA_TARGET_DIRECTORY: &str = "./crates/chronicle-synth/domain-schema";

const EXCLUDE_LIST: &str = "./crates/chronicle-synth/exclude_collections.json";

#[derive(Deserialize, Serialize)]
struct ExcludeCollections {
    exclude: Vec<String>,
}

impl ExcludeCollections {
    fn from_file(filename: impl AsRef<Path>) -> Result<ExcludeCollections, ChronicleSynthError> {
        let file = File::open(filename)?;
        let reader = BufReader::new(file);
        let exclude_collections = serde_json::from_reader(reader)?;
        Ok(exclude_collections)
    }
}

fn write_collection(collection: &Collection, dir_path: &Path) -> Result<(), ChronicleSynthError> {
    let file_path = dir_path.join(collection.path());
    let mut file = File::create(file_path)?;
    let schema = collection.json_schema()?;
    file.write_all(serde_json::to_string(&schema)?.as_bytes())?;
    Ok(())
}

/// Appends a collection name to the "exclude list" file, a list of collection files to be ignored
/// when generating the domain schema. See `generate` script in this repository for more information.
fn append_to_exclude_list(
    path: impl AsRef<Path>,
    collection: &str,
) -> Result<(), ChronicleSynthError> {
    let collection = collection.to_string();
    let mut list = ExcludeCollections::from_file(&path)?;

    if list.exclude.contains(&collection) {
        return Ok(());
    } else {
        list.exclude.push(collection);
    }

    let mut file = File::create(&path)?;
    file.write_all(serde_json::to_string_pretty(&list)?.as_bytes())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs::File, io::BufReader};

    use assert_fs::prelude::*;

    const PATH: &str = "test_exclude_collections.json";

    fn create_test_exclude_collections(
    ) -> Result<assert_fs::NamedTempFile, Box<dyn std::error::Error>> {
        let file = assert_fs::NamedTempFile::new(PATH)?;

        file.write_str(
            r#"
            {
                "exclude": [
                    "already_ignore_this"
                ]
            }
            "#,
        )?;

        Ok(file)
    }

    #[test]
    fn test_append_to_exclude_list() -> Result<(), ChronicleSynthError> {
        let file = create_test_exclude_collections().unwrap();

        // Call the function to append to the exclude list
        append_to_exclude_list(file.path(), "ignore_this_collection_when_printing")?;

        // Read the contents of the file and check if the collection was added
        let file = File::open(file.path())?;
        let reader = BufReader::new(file);
        let exclude_collections: ExcludeCollections = serde_json::from_reader(reader)?;

        insta::assert_json_snapshot!(exclude_collections, @r###"
        {
          "exclude": [
            "already_ignore_this",
            "ignore_this_collection_when_printing"
          ]
        }"###);

        Ok(())
    }

    #[test]
    fn test_append_to_exclude_list_skips_collections_already_on_list(
    ) -> Result<(), ChronicleSynthError> {
        let file = create_test_exclude_collections().unwrap();

        // Call the function to append to the exclude list
        append_to_exclude_list(file.path(), "already_ignore_this")?;

        // Read the contents of the file and check if the collection was added
        let file = File::open(file.path())?;
        let reader = BufReader::new(file);
        let exclude_collections: ExcludeCollections = serde_json::from_reader(reader)?;

        insta::assert_json_snapshot!(exclude_collections, @r###"
        {
          "exclude": [
            "already_ignore_this"
          ]
        }"###);

        Ok(())
    }
}
