# Importing existing Data into Chronicle

## Importing Data into Chronicle

Chronicle provides a CLI command `import` to load data from a JSON-LD file that
contains an array of Chronicle Operations. These operations can be used to
create agents, namespaces, and other resources in the Chronicle database. For
more information about the data format of the import file, see [Recording Provenance](../recording_provenance/#importing-data-into-chronicle).

## Example import process

### Local development environment

To import data into Chronicle, follow these steps:

- Create a JSON-LD file with an array of Chronicle Operations.
- Save the file in the Chronicle root directory with a unique name, for example,
  `import.json`.
- Run the following command:

  ```bash
  chronicle --bin chronicle -- import <namespace-id> <namespace-uuid> <json-file-url>
  ```

See the [CLI `import` command help](../cli#import-namespace-id-namespace-uuid-url)
for more information on each argument in the command.

If the data is successfully imported, the Chronicle Operations will be added
to the Chronicle database.

### Docker compose environment

Note: This example assumes you are using the chronicle-examples and running the
stl version of the docker compose file. You will need to adapt the instructions
to your environment, specifically the name of the chronicle container.

To import data into Chronicle, follow these steps:

- Create a JSON-LD file with an array of Chronicle Operations.
- Add a bind mount to your docker compose file mounting your file in the
  chronicle container. If you're using the [chronicle-examples](https://github.com/btpworks/chronicle-examples/)
  or [chronicle-bootstrap](https://github.com/btpworks/chronicle-bootstrap)
  configuration then this will be for the `chronicle-sawtooth-api` container,
  as defined in  `docker/chronicle.yaml`.

Note: You will need to adapt the instructions to your environment, specifically
the path to your import source file. The following example assumes that you
are using the [chronicle-examples](https://github.com/btpworks/chronicle-examples/)
repository and that your import file is in the root directory of the repository.

  ```yaml
  volumes:
    - type: bind
      source: ../import.json
      target: /import.json
  ```

- Start your docker compose environment
- Exec into the chronicle container

  ```bash
  docker exec -it chronicle bash
  ```

- Run the following command:

  ```bash
    chronicle -c /etc/chronicle/config/config.toml \
    --console-logging pretty \
    --sawtooth tcp://validator:4004 \
    import <namespace-id> <namespace-uuid> file:///import.json
  ```

See the  [CLI `import` command help](../cli#import-namespace-id-namespace-uuid-url)
for more information on each argument in the command.

If the data is successfully imported, the Chronicle Operations will be added
to the Chronicle database.
