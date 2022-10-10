FROM rust:latest as chef

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
  g++-x86-64-linux-gnu \
  g++-aarch64-linux-gnu \
  gcc-x86-64-linux-gnu \
  gcc-aarch64-linux-gnu \
  libzmq3-dev \
  libssl-dev \
  protobuf-compiler \
  && \
  apt-get clean && rm -rf /var/lib/apt/lists/*


FROM chef AS chronicle-builder
COPY . .
RUN cargo build --release
