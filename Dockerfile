FROM lukemathwalker/cargo-chef AS chef
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
FROM builder AS application
# Build application
COPY . .
RUN cargo build --release 

FROM debian:buster-slim AS chronicle
WORKDIR /app
COPY --from=application /app/target/release/chronicle /usr/local/bin
ENTRYPOINT ["/usr/local/bin/chronicle"]

FROM debian:buster-slim AS chronicle_sawtooth_tp
WORKDIR /app
COPY --from=application /app/target/release/chronicle_sawtooth_tp /usr/local/bin
ENTRYPOINT ["/usr/local/bin/chronicle_sawtooth_tp"]