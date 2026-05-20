FROM rust:1-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 1000 app
WORKDIR /app
COPY --from=builder /app/target/release/webwatch /usr/local/bin/webwatch
COPY config.toml.example /app/config.toml.example
RUN mkdir -p /data && chown -R app:app /app /data
USER app
ENV RUST_LOG=webwatch=info,tower_http=info \
    WEBWATCH_CONFIG=/app/config.toml
EXPOSE 3000
CMD ["webwatch"]
