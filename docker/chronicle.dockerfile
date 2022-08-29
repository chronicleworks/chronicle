FROM rust:latest as chef

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
#RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
#COPY --from=planner /app/recipe.json recipe.json
# This is the layer cached by cargo dependency
#RUN cargo chef cook --release --recipe-path recipe.json
FROM builder AS test
COPY . .
RUN cargo test --release

FROM builder AS chronicle-base
# Build tp
COPY . .

ARG BUILD_ARGS
RUN cargo build --release ${BUILD_ARGS}

FROM ubuntu:focal AS chronicle-tp
WORKDIR /
COPY --from=chronicle-base /app/target/release/chronicle_sawtooth_tp /usr/local/bin

RUN apt-get update -yq \
  && apt-get install --no-install-recommends -yq \
  sqlite3 \
  && apt-get upgrade -yq --no-install-recommends \
  && apt-get autoremove -yq \
  && apt-get autoclean -yq \
  && apt-get clean \
  && rm -rf /var/lib/apt/lists/* \
  && rm -rf /tmp/*
