FROM messense/rust-musl-cross:x86_64-musl

ENV DEBIAN_FRONTEND=noninteractive
ENV PKG_CONFIG_ALLOW_CROSS=1 
ENV OPENSSL_STATIC=true


RUN apt-get update && \
    apt-get install -y \
    libzmq3-dev \
    libssl-dev \
    protobuf-compiler \
    && \
    apt-get clean && rm -rf /var/lib/apt/lists/*

RUN rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl

