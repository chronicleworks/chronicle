FROM rust:latest as builder

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


FROM builder AS test
COPY . .
RUN cargo test --release

FROM builder AS base
# Build tp
COPY . .

ARG BUILD_ARGS
RUN cargo build --release ${BUILD_ARGS}

FROM ubuntu:focal AS chronicle-tp
WORKDIR /
COPY --from=base /app/target/release/chronicle_sawtooth_tp /usr/local/bin
