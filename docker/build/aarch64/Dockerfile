FROM messense/rust-musl-cross:aarch64-musl

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && \
    apt-get install -y \
    libzmq3-dev \
    libssl-dev \
    && \
    apt-get clean && rm -rf /var/lib/apt/lists/*

