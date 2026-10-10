FROM rust:latest AS builder

ARG RELEASE_MODE=false

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
COPY .mise/config.toml .mise/config.toml
COPY .mise/mise.lock .mise/mise.lock
RUN mise install protoc

COPY ./ ./
RUN --mount=type=cache,uid=1000,gid=1000,id=cargo-registry,target=/home/builder/.cargo/registry \
    --mount=type=cache,uid=1000,gid=1000,id=cargo-git,target=/home/builder/.cargo/git \
    --mount=type=cache,uid=1000,gid=1000,id=cargo-target,target=/build/target \
    if [ "$RELEASE_MODE" = "true" ]; then \
        RELEASE_FLAG="--release"; \
        TARGET_DIR="release"; \
    else \
        RELEASE_FLAG=""; \
        TARGET_DIR="debug"; \
    fi &&\
    SQLX_OFFLINE=true mise exec protoc -- cargo build ${RELEASE_FLAG} -p game_master \
    && mkdir -p /build/bin/ \
    && cp /build/target/${TARGET_DIR}/game_master /build/bin/game_master

FROM debian:latest

RUN apt update && apt install -y ca-certificates && \
    update-ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/bin/game_master /app/game_master

ENTRYPOINT [ "/app/game_master" ]
