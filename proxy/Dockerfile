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

# Create a minimal frontend/Cargo.toml to satisfy workspace
RUN mkdir -p frontend && echo '[package]\nname = "placeholder"\nversion = "0.1.0"\nedition = "2021"' > frontend/Cargo.toml

# Copy proxy source
COPY proxy ./proxy

# Build only the proxy
RUN cd proxy && cargo build --release --bin bird-lgproxy-rs

# Runtime stage - Alpine
FROM alpine:3.20

# Install runtime dependencies
RUN apk add --no-cache \
    mtr \
    iputils \
    bind-tools \
    ca-certificates \
    traceroute

# Copy the binary
COPY --from=builder /app/proxy/target/release/bird-lgproxy-rs /usr/local/bin/bird-lgproxy-rs

# Create non-root user
RUN addgroup -g 1000 -S birdlg && \
    adduser -u 1000 -S birdlg -G birdlg

USER birdlg

EXPOSE 8000

CMD ["bird-lgproxy-rs"] 