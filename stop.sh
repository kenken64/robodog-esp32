#!/bin/bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY="$SCRIPT_DIR/target/release/wifi-proxy"

# Default values
INTERFACE="wlxdceae760e328"
PORT=8080

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m'

print_status() {
    echo -e "${CYAN}[*]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[+]${NC} $1"
}

# Kill server process
if lsof -ti:$PORT >/dev/null 2>&1; then
    print_status "Stopping server on port $PORT..."
    lsof -ti:$PORT | xargs kill -9 2>/dev/null || true
    print_success "Server stopped"
else
    print_status "No server running on port $PORT"
fi

# Disconnect WiFi
if [ -f "$BINARY" ]; then
    print_status "Disconnecting interface $INTERFACE..."
    "$BINARY" disconnect --interface "$INTERFACE" 2>/dev/null && \
        print_success "Disconnected" || \
        print_status "Interface was not connected"
fi
