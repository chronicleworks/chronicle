//! This module provides utilities for importing and exporting data to and from the Chronicle API.

use std::{
    fs::File,
    io::{Read, Write},
    path::PathBuf,
};

use thiserror::Error;
use tracing::{debug, error, instrument};

use crate::prov::{operations::ChronicleOperation, to_json_ld::ToJson};

#[derive(Error, Debug)]
pub enum ETLError {
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),

    #[error("Malformed JSON: {0}")]
    SerdeJson(#[from] serde_json::Error),
}

fn read_file_to_string(path: impl AsRef<std::path::Path>) -> Result<String, ETLError> {
    let mut file = std::fs::File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

#[instrument]
pub fn read_and_update_operations_from_file(
    tx: &Vec<ChronicleOperation>,
    path: &PathBuf,
) -> Result<Vec<serde_json::Value>, ETLError> {
    debug!(
        "Reading {:?} in working directory {:?}",
        path,
        std::env::current_dir()
    );

    let contents = read_file_to_string(path)?;

    // Parse JSON and get array
    let json = serde_json::from_str::<serde_json::Value>(&contents).unwrap_or_else(|e| {
        error!(
            "Error parsing JSON operations data: {}. File export.json may not exist",
            e
        );
        serde_json::Value::Array(vec![])
    });

    let mut operations: Vec<serde_json::Value> = match json {
        serde_json::Value::Array(arr) => {
            debug!("Parsed JSON array from export.json: {:?}", arr);
            arr
        }
        _ => {
            debug!("JSON was not an array");
            vec![]
        }
    };

    // Append operations to the `operations` vector
    tx.iter().for_each(|op| {
        operations.extend(op.to_json().0.as_array().unwrap_or(&vec![]).iter().cloned());
    });

    Ok(operations)
}

#[instrument]
pub fn write_operations_to_file(
    operations: &[serde_json::Value],
    path: &PathBuf,
) -> Result<(), ETLError> {
    // Open file for writing and write updated JSON
    let mut file = File::create(path)?;
    file.write_all(serde_json::to_string_pretty(&operations)?.as_bytes())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::{
        attributes::{Attribute, Attributes},
        prov::{
            operations::{AgentExists, SetAttributes},
            AgentId, DomaintypeId, ExternalId, NamespaceId,
        },
    };

    use super::*;

    fn test_create_agent_operations_import() -> assert_fs::NamedTempFile {
        let file = assert_fs::NamedTempFile::new("import.json").unwrap();
        assert_fs::prelude::FileWriteStr::write_str(
            &file,
            r#"
        [
            {
                "@id": "_:n1",
                "@type": [
                "http://btp.works/chronicleoperations/ns#CreateNamespace"
                ],
                "http://btp.works/chronicleoperations/ns#namespaceName": [
                {
                    "@value": "testns"
                }
                ],
                "http://btp.works/chronicleoperations/ns#namespaceUuid": [
                {
                    "@value": "6803790d-5891-4dfa-b773-41827d2c630b"
                }
                ]
            }
        ]
         "#,
        )
        .unwrap();
        file
    }

    #[test]
    fn test_read_file_to_string() {
        let file = test_create_agent_operations_import();
        let contents = read_file_to_string(file.path()).unwrap();
        let json = serde_json::from_str::<serde_json::Value>(&contents).unwrap();
        insta::assert_json_snapshot!(json, @r###"
        [
          {
            "@id": "_:n1",
            "@type": [
              "http://btp.works/chronicleoperations/ns#CreateNamespace"
            ],
            "http://btp.works/chronicleoperations/ns#namespaceName": [
              {
                "@value": "testns"
              }
            ],
            "http://btp.works/chronicleoperations/ns#namespaceUuid": [
              {
                "@value": "6803790d-5891-4dfa-b773-41827d2c630b"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_read_and_update_operations_from_file() {
        let file = test_create_agent_operations_import();

        let namespace = NamespaceId::from_external_id(
            "testns",
            Uuid::parse_str("6803790d-5891-4dfa-b773-41827d2c630b").unwrap(),
        );

        let create = ChronicleOperation::AgentExists(AgentExists {
            external_id: ExternalId::from("testagent"),
            namespace: namespace.clone(),
        });

        let id = AgentId::from_external_id("testagent");
        let attributes = Attributes {
            typ: Some(DomaintypeId::from_external_id("test")),
            attributes: [(
                "test".to_owned(),
                Attribute {
                    typ: "test".to_owned(),
                    value: serde_json::Value::String("test".to_owned()),
                },
            )]
            .into_iter()
            .collect(),
        };
        let set_type = ChronicleOperation::SetAttributes(SetAttributes::Agent {
            id,
            namespace,
            attributes,
        });

        let operations =
            read_and_update_operations_from_file(&vec![create, set_type], &file.path().into())
                .unwrap();

        write_operations_to_file(&operations, &file.path().into()).unwrap();

        let data = read_file_to_string(file.path()).unwrap();
        let json = serde_json::from_str::<serde_json::Value>(&data).unwrap();

        insta::assert_json_snapshot!(json, @r###"
        [
          {
            "@id": "_:n1",
            "@type": [
              "http://btp.works/chronicleoperations/ns#CreateNamespace"
            ],
            "http://btp.works/chronicleoperations/ns#namespaceName": [
              {
                "@value": "testns"
              }
            ],
            "http://btp.works/chronicleoperations/ns#namespaceUuid": [
              {
                "@value": "6803790d-5891-4dfa-b773-41827d2c630b"
              }
            ]
          },
          {
            "@id": "_:n1",
            "@type": [
              "http://btp.works/chronicleoperations/ns#AgentExists"
            ],
            "http://btp.works/chronicleoperations/ns#agentName": [
              {
                "@value": "testagent"
              }
            ],
            "http://btp.works/chronicleoperations/ns#namespaceName": [
              {
                "@value": "testns"
              }
            ],
            "http://btp.works/chronicleoperations/ns#namespaceUuid": [
              {
                "@value": "6803790d-5891-4dfa-b773-41827d2c630b"
              }
            ]
          },
          {
            "@id": "_:n1",
            "@type": [
              "http://btp.works/chronicleoperations/ns#SetAttributes"
            ],
            "http://btp.works/chronicleoperations/ns#agentName": [
              {
                "@value": "testagent"
              }
            ],
            "http://btp.works/chronicleoperations/ns#attributes": [
              {
                "@type": "@json",
                "@value": {
                  "test": "test"
                }
              }
            ],
            "http://btp.works/chronicleoperations/ns#domaintypeId": [
              {
                "@value": "test"
              }
            ],
            "http://btp.works/chronicleoperations/ns#namespaceName": [
              {
                "@value": "testns"
              }
            ],
            "http://btp.works/chronicleoperations/ns#namespaceUuid": [
              {
                "@value": "6803790d-5891-4dfa-b773-41827d2c630b"
              }
            ]
          }
        ]
        "###);
    }
}
