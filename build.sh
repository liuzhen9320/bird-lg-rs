#!/bin/bash

# Build script for Bird LG Docker images using unified Dockerfile

set -e

echo "Building Bird LG Docker images..."

# Build proxy image
echo "Building proxy image..."
docker build \
    --target proxy-runtime \
    --build-arg SERVICE=proxy \
    -t bird-lg-proxy:latest \
    .

# Build frontend image  
echo "Building frontend image..."
docker build \
    --target frontend-runtime \
    --build-arg SERVICE=frontend \
    -t bird-lg-frontend:latest \
    .

echo "✅ Both images built successfully!"
echo "🚀 To run with docker-compose: docker-compose up"
echo "📦 Images created:"
echo "   - bird-lg-proxy:latest"
echo "   - bird-lg-frontend:latest"

# Optional: Show image sizes
echo ""
echo "📊 Image sizes:"
docker images | grep bird-lg 