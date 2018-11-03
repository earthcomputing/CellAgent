FROM rust:latest

# http://whitfin.io/speeding-up-rust-docker-builds/

RUN apt-get update && \
    apt-get dist-upgrade -y && \
    apt-get install -y csh colordiff less vim && \
    rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/* && \
    apt-get clean

RUN USER=root cargo new --bin /usr/src/CellAgent
WORKDIR /usr/src/CellAgent

COPY Cargo.lock Cargo.lock
COPY Cargo.toml Cargo.toml

ENV RUSTFLAGS "-C debuginfo=2 -A dead_code -A unused-variables -A unused-imports -A non-snake-case"

# build dependencies (will be cached)
# creates a "hello world" template
RUN cargo build --release

RUN rm src/main.rs
COPY src src
# RUN cargo build --release
RUN cargo install

# CMD ["multicell"]
