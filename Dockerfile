ARG APP_NAME=gale-bot

FROM rust:1.96.0-slim-trixie AS build
ARG APP_NAME
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev

RUN --mount=type=bind,source=src,target=src \
    --mount=type=bind,source=Cargo.toml,target=Cargo.toml \
    --mount=type=bind,source=Cargo.lock,target=Cargo.lock \
    --mount=type=cache,target=/app/target/ \
    --mount=type=cache,target=/var/cache/cargo \
    CARGO_HOME=/var/cache/cargo cargo build --locked --release && \
    cp ./target/release/$APP_NAME /bin/server

FROM debian:trixie-slim AS final

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /bin/server /bin/
COPY ./gale-bot-production.toml /etc/gale-bot/config.toml

EXPOSE 8080

ENTRYPOINT ["/bin/server"]
