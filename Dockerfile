FROM rust:1.85-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked
FROM debian:bookworm-slim
RUN useradd --system --create-home app
COPY --from=build /src/target/release/trust-boundaries-demo /usr/local/bin/trust-boundaries-demo
USER app
ENTRYPOINT ["/usr/local/bin/trust-boundaries-demo"]
