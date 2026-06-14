# Mehrstufiger Build: erst übersetzen, dann ein winziges Laufzeit-Image.

# --- Build-Stufe (volles Rust-Image, enthält den C-Compiler für SQLite) ---
FROM rust:1-bookworm AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

# --- Laufzeit-Stufe (distroless: kein Shell, kleine Angriffsfläche) ---
FROM gcr.io/distroless/cc-debian12 AS runtime
# Nicht-root (UID/GID 10001, passend zur docker-compose.yml).
USER 10001:10001
COPY --from=build /app/target/release/antrag3000-server /antrag3000-server
ENTRYPOINT ["/antrag3000-server"]
