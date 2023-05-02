# Chronicle Synth

Chronicle Domain [Synth](https://www.getsynth.com/) is a program for
generating Synth schema for Chronicle. It takes a YAML file as input,
which contains information about the domain's name, attributes,
entities, activities, agents, and roles, and generates Synth collections
that can be used to generate the Chronicle operations available to a
domain's users.

If you just want to generate untyped Chronicle operations with `synth`
the collections in `crates/chronicle-synth/synth/` are ready to go.

The core functionality of Chronicle Synth is to generate the
additional Synth schema specific to each individual Chronicle domain.
Replace the `domain.yaml` file in `crates/chronicle-synth/` with your
domain and run

```bash
cargo run --bin chronicle
```

Run the `generate` script to generate a set of each operation available
to your domain:

```bash
cd crates/chronicle-synth && \
./generate
```

## Prerequisites

To use Chronicle Synth, you need to have Synth installed on
your system. If you don't have it already installed, you can follow
the [instructions](https://www.getsynth.com/docs/getting_started/installation)
in the Synth documentation to install it.

## Usage

### Loading domain.yaml Chronicle Domain Definitions

To use Chronicle Synth, you must have a YAML file that
describes your domain. The file should have the following structure:

```yaml
name: DomainName
attributes:
  AttributeName:
    type: AttributeType
entities:
  EntityName:
    attributes:
      - AttributeName1
      - AttributeName2
activities:
  ActivityName:
    attributes:
      - AttributeName1
      - AttributeName2
agents:
  AgentName:
    attributes:
      - AttributeName1
roles:
  - RoleName1
  - RoleName2
```

Once you have your YAML file, you can generate Synth schema by running
the following command:

```bash
chronicle-synth <path-to-yaml-file>
```

Or, since `<path-to-yaml-file>` defaults to ./crates/synth/domain.yaml,
if your YAML file is named domain.yaml, you can simply run:

```bash
cargo run --bin chronicle-synth --
```

This will output the additional Synth schema required to generate
synth data for your domain to the `domain_schema` directory, and collate
the core Chronicle Synth collections along with the generated collections
specific to your domain in `collections`.

### Generating Your Domain's Synth Data

Chronicle Synth includes a `generate` script, which will print
a synth-example of each Chronicle operation in your domain.

#### Example

```bash
./crates/chronicle-synth/generate | jq
```
