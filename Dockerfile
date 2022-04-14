FROM rust:buster AS chef
# We only pay the installation cost once,
# it will be cached from the second build onwards
RUN cargo install cargo-chef

WORKDIR /app

ENV DEBIAN_FRONTEND=noninteractive
ENV PKG_CONFIG_ALLOW_CROSS=1
ENV OPENSSL_STATIC=true

RUN apt-get update && \
    apt-get install -y \
    build-essential \
    cmake \
    libzmq3-dev \
    libssl-dev \
    protobuf-compiler \
    && \
    apt-get clean && rm -rf /var/lib/apt/lists/*


FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# This is the layer cached by cargo dependency
RUN cargo chef cook --release --recipe-path recipe.json
FROM builder AS test
# Build application
COPY . .
RUN cargo test --release

FROM builder AS application
# Build application
COPY . .
RUN cargo build --release

FROM ubuntu:bionic AS chronicle
WORKDIR /
COPY --from=application /app/target/release/chronicle /usr/local/bin
COPY --from=application /app/target/release/chronicle_sawtooth_tp /usr/local/bin
