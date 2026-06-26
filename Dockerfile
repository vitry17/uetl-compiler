FROM rust:1.78 AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/uetl-compiler /usr/local/bin/uetl-compiler
EXPOSE 4001
CMD ["uetl-compiler"]
