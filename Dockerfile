# Unified Dockerfile for Bird LG - both proxy and frontend
# Usage: 
#   docker build --target proxy-runtime --build-arg SERVICE=proxy -t bird-lg-proxy .
#   docker build --target frontend-runtime --build-arg SERVICE=frontend -t bird-lg-frontend .

ARG SERVICE=proxy

# Build stage - Alpine with Rust
FROM alpine:3.20 AS builder

# Install build dependencies
RUN apk add --no-cache \
    curl \
    build-base \
    openssl-dev \
    pkgconfig \
    musl-dev

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.87.0
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./

# Copy all source code
COPY proxy ./proxy
COPY frontend ./frontend

# Build the appropriate service
ARG SERVICE
RUN echo "Building service: $SERVICE" && \
    if [ "$SERVICE" = "proxy" ]; then \
        echo "Building proxy..." && \
        cd proxy && \
        cargo build --release --bin bird-lgproxy-rs && \
        ls -la target/release/ && \
        echo "Proxy build completed"; \
    elif [ "$SERVICE" = "frontend" ]; then \
        echo "Building frontend..." && \
        cd frontend && \
        cargo build --release --bin bird-lg-rs && \
        ls -la target/release/ && \
        echo "Frontend build completed"; \
    else \
        echo "Unknown service: $SERVICE" && exit 1; \
    fi

# Proxy runtime stage
FROM alpine:3.20 AS proxy-runtime

# Install runtime dependencies for proxy
RUN apk add --no-cache \
    mtr \
    iputils \
    bind-tools \
    ca-certificates \
    traceroute

# Copy the proxy binary
COPY --from=builder /app/proxy/target/release/bird-lgproxy-rs /usr/local/bin/bird-lgproxy-rs

# Create non-root user
RUN addgroup -g 1000 -S birdlg && \
    adduser -u 1000 -S birdlg -G birdlg

USER birdlg

EXPOSE 8000

CMD ["bird-lgproxy-rs"]

# Frontend runtime stage
FROM alpine:3.20 AS frontend-runtime

# Install runtime dependencies for frontend
RUN apk add --no-cache \
    ca-certificates

# Copy the frontend binary
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