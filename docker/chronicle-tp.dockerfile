FROM --platform=$BUILDPLATFORM rust:latest as builder

ARG BUILDPLATFORM
ARG TARGETPLATFORM
ARG TARGETARCH
ARG BUILD_ARGS

WORKDIR /app

ENV DEBIAN_FRONTEND=noninteractive
ENV PKG_CONFIG_ALLOW_CROSS=1
ENV OPENSSL_STATIC=true
ENV PROTOCOL_BUF_VERSION=v3.15.1

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

RUN if [ "$TARGETARCH" = "amd64" ]; then \
  TARGET=x86_64-unknown-linux-gnu; \
  elif [ "$TARGETARCH" = "arm64" ]; then \
  TARGET=aarch64-unknown-linux-gnu; \
  else \
  echo "Unsupported architecture: $TARGETARCH"; \
  exit 1; \
  fi \
  && rustup target add "${TARGET}"

FROM builder AS test
COPY . .
RUN if [ "$TARGETARCH" = "amd64" ]; then \
  TARGET=x86_64-unknown-linux-gnu; \
  elif [ "$TARGETARCH" = "arm64" ]; then \
  TARGET=aarch64-unknown-linux-gnu; \
  else \
  echo "Unsupported architecture: $TARGETARCH"; \
  exit 1; \
  fi \
  && cargo clean \
  && cargo build --target "${TARGET}" --release ${BUILD_ARGS} \
  --bin chronicle_sawtooth_tp \
  && mv -f "target/${TARGET}" "target/${TARGETARCH}"

FROM --platform=$TARGETPLATFORM ubuntu:focal AS chronicle
ARG TARGETARCH
WORKDIR /
COPY --from=test /app/target/${TARGETARCH}/release/chronicle_sawtooth_tp /usr/local/bin
