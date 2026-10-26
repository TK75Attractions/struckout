FROM ubuntu:latest

COPY --from=ghcr.io/jdx/mise:latest /usr/local/bin/mise /usr/bin/mise

RUN apt update && apt install -y curl clang git \
    && rm -rf /var/lib/apt/lists/*

COPY ./mise.toml ./mise.toml
COPY ./mise.lock ./mise.lock

RUN mise install --locked
