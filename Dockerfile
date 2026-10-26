FROM rust:1.86 AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
# Compile d'abord les dépendances seules, avec des sources factices : tant
# que Cargo.toml/Cargo.lock ne changent pas, Docker réutilise ce calque en
# cache même quand le vrai code source change — sans ça, modifier une seule
# ligne dans src/ invalidait tout le cache et recompilait chaque dépendance
# (axum, tokio, etc.) à chaque build, l'essentiel du temps de build.
RUN mkdir -p src benches \
    && echo "fn main() {}" > src/main.rs \
    && touch src/lib.rs \
    && echo "fn main() {}" > benches/compile.rs \
    && cargo build --release \
    && rm -rf src benches
COPY src ./src
COPY benches ./benches
# Sans ce `touch`, cargo peut voir les timestamps copiés comme "à jour" par
# rapport au binaire déjà construit à partir des sources factices, et sauter
# la recompilation du vrai code.
RUN touch src/main.rs src/lib.rs benches/compile.rs && cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --no-create-home --uid 10001 uetl
COPY --from=builder /app/target/release/uetl-compiler /usr/local/bin/uetl-compiler
USER uetl
EXPOSE 4001
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:4001/health || exit 1
CMD ["uetl-compiler"]
