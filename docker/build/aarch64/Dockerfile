FROM messense/rust-musl-cross:aarch64-musl

ENV DEBIAN_FRONTEND=noninteractive
ENV PKG_CONFIG_ALLOW_CROSS=1 

RUN apt-get update && \
    apt-get install -y \
    libzmq3-dev \
    libssl-dev \
    protobuf-compiler \
    && \
    apt-get clean && rm -rf /var/lib/apt/lists/*

