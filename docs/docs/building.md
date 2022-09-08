
# Building Chronicle

BTP maintain and distribute a docker image for the [chronicle transaction processor](./chronicle_architecture.md#transaction-processor). End users of chronicle need to build their own versions to support their custom domains. BTP maintains and distributes a docker build image to be used in CI/CD.

## Example dockerfile

Assuming a valid [chronicle domain configuration](./domain_modelling.md) located in the same directory as the dockerfile, the following will build a domain specific chronicle. You should only need to source control the Dockerfile and domain.config - chronicle's build image will do the rest.

``` docker
FROM blockchaintp/builder:{VERSION_NUMBER} as domain

COPY domain.yaml chronicle-domain/
cargo build --release --frozen --bin chronicle

```

## In memory version

For rapids development and testing purposes a standalone version of Chronicle can be built and distributed as a docker image or binary.

``` docker
FROM blockchaintp/builder:{VERSION_NUMBER} as domain

COPY domain.yaml chronicle-domain/
cargo build --release --frozen --features inmem --bin chronicle

```
