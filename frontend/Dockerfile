# Build stage
FROM rust:1.87 as builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY frontend ./frontend

RUN cd frontend && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/frontend/target/release/bird-lg-rs /usr/local/bin/bird-lg-rs
COPY frontend/assets /app/assets

WORKDIR /app

EXPOSE 5000

CMD ["bird-lg-rs"] 