FROM rust:latest AS builder

ENV CARGO_HOME="/home/builder/.cargo/"
ENV PATH="/home/builder/.local/bin/:${PATH}"

RUN apt update && apt install -y curl \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd --system builder --gid 1000 \
    && useradd --system --create-home -g builder --uid 1000 builder
USER builder

# install protoc via mise
RUN curl https://mise.run|sh

WORKDIR /build/

# mise.tomlの変更頻度は低いので先にコピーしてインストールしておく
COPY mise.toml mise.toml
RUN mise install

COPY ./ ./
RUN --mount=type=cache,uid=1000,gid=1000,id=cargo-registry,target=/home/builder/.cargo/registry \
    --mount=type=cache,uid=1000,gid=1000,id=cargo-git,target=/home/builder/.cargo/git \
    --mount=type=cache,uid=1000,gid=1000,id=cargo-target,target=/build/target \
    mise exec -- cargo build --release -p game_master \
    && mkdir -p /build/bin/ \
    && cp /build/target/release/game_master /build/bin/game_master

FROM debian:latest

COPY --from=builder /build/bin/game_master /app/game_master

ENTRYPOINT [ "/app/game_master" ]
