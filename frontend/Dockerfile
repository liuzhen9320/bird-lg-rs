# Build stage - Alpine with Rust
FROM alpine:3.20 AS builder

# Install build dependencies
RUN apk add --no-cache \
    curl \
    build-base \
    openssl-dev \
    pkgconfig

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.87.0
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./

# Create a minimal proxy/Cargo.toml to satisfy workspace
RUN mkdir -p proxy && echo '[package]\nname = "placeholder"\nversion = "0.1.0"\nedition = "2021"' > proxy/Cargo.toml

# Copy frontend source
COPY frontend ./frontend

# Build only the frontend
RUN cd frontend && cargo build --release --bin bird-lg-rs

# Runtime stage - Alpine
FROM alpine:3.20

# Install runtime dependencies
RUN apk add --no-cache \
    ca-certificates

# Copy the binary
COPY --from=builder /app/frontend/target/release/bird-lg-rs /usr/local/bin/bird-lg-rs

# Copy assets
COPY frontend/assets /app/assets

# Create non-root user
RUN addgroup -g 1000 -S birdlg && \
    adduser -u 1000 -S birdlg -G birdlg && \
    chown -R birdlg:birdlg /app

USER birdlg
WORKDIR /app

EXPOSE 5000

CMD ["bird-lg-rs"] 