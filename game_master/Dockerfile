FROM rust:latest AS builder

WORKDIR /build/

COPY ./ ./

RUN cargo build -p game-master

FROM alpine:latest

COPY --from=builder /build/target/debug/game_master /app/game_master

ENTRYPOINT [ "/app/game-master" ]
