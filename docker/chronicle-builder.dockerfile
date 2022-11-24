FROM --platform=$TARGETPLATFORM rust:latest as chef

ARG BUILDPLATFORM
ARG TARGETPLATFORM
ARG TARGETARCH
ARG BUILD_ARGS

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

COPY . .

RUN cargo fetch --locked
RUN cargo build --release --locked --bin chronicle-domain-lint
RUN cp target/release/chronicle-domain-lint /usr/local/bin
RUN cargo clean
