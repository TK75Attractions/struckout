FROM rust:latest AS builder

ARG RELEASE_MODE=false
ARG SENTRY_DSN

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
        if [ "${SENTRY_DSN}" = "" ]; then \
            echo "RELEASE_MODE is set to true, but SENTRY_DSN is not supplied"; \
            exit 1; \
        fi &&\
        FLAGS="--release --features=sentry"; \
        TARGET_DIR="release"; \
    else \
        if [ "${SENTRY_DSN}" != "" ]; then \
            FLAGS="--features=sentry"; \
        else \
            FLAGS=""; \
        fi &&\
        TARGET_DIR="debug"; \
    fi &&\
    SQLX_OFFLINE=true mise exec protoc -- cargo build ${FLAGS} -p game_master \
    && mkdir -p /build/bin/ \
    && cp /build/target/${TARGET_DIR}/game_master /build/bin/game_master

FROM debian:latest

RUN apt update && apt install -y ca-certificates && \
    update-ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/bin/game_master /app/game_master

ENTRYPOINT [ "/app/game_master" ]
