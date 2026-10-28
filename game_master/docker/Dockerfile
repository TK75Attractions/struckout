FROM rust:1.99.1-trixie AS base

ENV CARGO_HOME="/home/builder/.cargo/"
ENV PATH="/home/builder/.local/bin/:${PATH}"

RUN apt update && apt install -y clang \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd --system builder --gid 1000 \
    && useradd --system --create-home -g builder --uid 1000 builder
USER builder

COPY --from=ghcr.io/jdx/mise:latest /usr/local/bin/mise /usr/bin/mise

WORKDIR /build/

# mise.tomlの変更頻度は低いので先にコピーしてインストールしておく
COPY mise.toml mise.toml
COPY mise.lock mise.lock
COPY game_master/mise.toml game_master/mise.toml
RUN mise install --locked protoc github:rui314/mold cargo:cargo-chef

FROM base AS planner
COPY ./ ./
RUN mise exec cargo:cargo-chef -- cargo chef prepare --recipe-path recipe.json

FROM base AS builder
COPY --from=planner /build/recipe.json recipe.json
COPY .cargo/config.toml .cargo/config.toml
COPY api/proto api/proto
COPY api/spec api/spec
ARG RELEASE_MODE=false
RUN if [ "$RELEASE_MODE" = "true" ]; then \
    FLAGS="--release --features=sentry"; \
    else \
    FLAGS=""; \
    fi && \
    SQLX_OFFLINE=true mise exec protoc mold cargo:cargo-chef -- \
    cargo chef cook --recipe-path recipe.json ${FLAGS} -p game_master

COPY ./ ./
ARG ST_VERSION
ARG ST_COMMIT_SHORT_HASH
RUN if [ "$RELEASE_MODE" = "true" ]; then \
    FLAGS="--release --features=sentry"; \
    TARGET_DIR="release"; \
    else \
    FLAGS=""; \
    TARGET_DIR="debug"; \
    fi && \
    SQLX_OFFLINE=true ST_VERSION=${ST_VERSION} ST_COMMIT_SHORT_HASH=${ST_COMMIT_SHORT_HASH} \
    mise exec protoc mold -- cargo build ${FLAGS} -p game_master \
    && mkdir -p /build/bin/ \
    && cp /build/target/${TARGET_DIR}/game_master /build/bin/game_master

FROM debian:trixie

RUN apt update && apt install -y ca-certificates && \
    update-ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/bin/game_master /app/game_master

ENTRYPOINT [ "/app/game_master" ]
