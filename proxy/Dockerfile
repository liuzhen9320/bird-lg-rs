# Build stage
FROM rust:1.87 as builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY proxy ./proxy

RUN cd proxy && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    traceroute \
    mtr-tiny \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/proxy/target/release/bird-lgproxy-rs /usr/local/bin/bird-lgproxy-rs

EXPOSE 8000

CMD ["bird-lgproxy-rs"] 