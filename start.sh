#!/bin/bash

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY="$SCRIPT_DIR/target/release/wifi-proxy"

# Default values
SSID="WAVESHARE Robot"
PASSWORD="1234567890"
INTERFACE="wlxdceae760e328"
PORT=8080

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

print_status() {
    echo -e "${CYAN}[*]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[+]${NC} $1"
}

print_error() {
    echo -e "${RED}[!]${NC} $1"
}

# Build if needed
if [ ! -f "$BINARY" ]; then
    print_status "Building release binary..."
    cargo build --release --manifest-path "$SCRIPT_DIR/Cargo.toml"
fi

# Kill any existing process on the port
if lsof -ti:$PORT >/dev/null 2>&1; then
    print_status "Killing existing process on port $PORT..."
    lsof -ti:$PORT | xargs kill -9 2>/dev/null || true
    sleep 1
fi

# Connect to WiFi
print_status "Connecting to '$SSID' on interface $INTERFACE..."
if "$BINARY" connect "$SSID" --password "$PASSWORD" --interface "$INTERFACE" 2>/dev/null; then
    print_success "Connected to $SSID"
else
    print_error "Failed to connect. Checking if already connected..."
    if "$BINARY" status --interface "$INTERFACE" 2>/dev/null | grep -q "$SSID"; then
        print_success "Already connected to $SSID"
    else
        print_error "Connection failed"
        exit 1
    fi
fi

# Start server
print_status "Starting web server on port $PORT..."
print_success "Access the control panel at: http://localhost:$PORT/"
echo ""

exec "$BINARY" serve --port "$PORT" --interface "$INTERFACE"
