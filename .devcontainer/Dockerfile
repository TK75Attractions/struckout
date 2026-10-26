FROM ubuntu:latest

COPY --from=ghcr.io/jdx/mise@sha256:1e0a39bc5a3b089b9b9305b6cd53e5d32ed5327e6241db5ec412fc2eabd44b02 /usr/local/bin/mise /usr/bin/mise

COPY ./mise.toml ./mise.toml
COPY ./mise.lock ./mise.lock

RUN mise install --locked
