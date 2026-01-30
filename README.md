# RoboDog ESP32 WiFi Proxy

A CLI tool to connect a secondary USB WiFi adapter to a different access point and proxy web requests to devices on that network. Designed for controlling ESP32-based robot dogs and similar IoT devices from your main computer.

## Features

- Connect a USB WiFi adapter to a separate network while keeping your main connection
- Web server that proxies HTTP requests and camera streams to the gateway
- Web-based control interface with keyboard and gamepad support
- Save network credentials for quick reconnection
- Scan for available WiFi networks

## Requirements

- Linux with NetworkManager
- A secondary USB WiFi adapter
- Rust toolchain (for building)

## Installation

```bash
cargo build --release
```

The binary will be at `target/release/wifi-proxy`.

## Quick Start

Use the convenience scripts to start/stop the server:

```bash
# Start the server (builds if needed, connects to WiFi, starts web server)
./start.sh

# Stop the server and disconnect
./stop.sh
```

Edit `start.sh` to configure your network settings (SSID, password, interface).

## CLI Usage

### List WiFi Interfaces

```bash
wifi-proxy list-interfaces
```

### Scan for Networks

```bash
wifi-proxy scan
wifi-proxy scan --interface wlan1
```

### Connect to a Network

```bash
wifi-proxy connect "SSID" --password "password"
wifi-proxy connect "SSID" --password "password" --interface wlan1 --save
```

### Check Connection Status

```bash
wifi-proxy status
wifi-proxy status --interface wlan1
```

### Disconnect

```bash
wifi-proxy disconnect
wifi-proxy disconnect --interface wlan1
```

### Start the Proxy Server

```bash
wifi-proxy serve --port 8080
wifi-proxy serve --port 8080 --interface wlan1
```

### Save Network Credentials

```bash
wifi-proxy save-network "SSID" --password "password"
wifi-proxy show-config
```

### Fetch Gateway Page

```bash
wifi-proxy fetch-gateway --output gateway.html
```

## Web Interface

Once the server is running, access the control panel at `http://localhost:8080/`.

### Keyboard Controls

| Key | Action |
|-----|--------|
| W / Arrow Up | Move forward |
| S / Arrow Down | Move backward |
| A / Arrow Left | Turn left |
| D / Arrow Right | Turn right |
| Space / Escape | Stop (Steady) |

### Gamepad Controls

Connect any standard gamepad to control the robot:

**Movement:**
- Left stick or D-pad: Forward, backward, left, right

**Actions:**
| Button | Action |
|--------|--------|
| A (0) | Steady |
| B (1) | Stay Low |
| X (2) | Hand Shake |
| Y (3) | Jump |
| LB (4) | Action A |
| RB (5) | Action B |
| LT (6) | Action C |
| RT (7) | Init Position |
| Select (8) | Middle Position |
| Start (9) | Toggle camera stream |

## Configuration

Credentials are stored in `~/.config/wifi-proxy/config.toml`:

```toml
[[networks]]
ssid = "WAVESHARE Robot"
password = "1234567890"
interface = "wlxdceae760e328"
```

## Architecture

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Your Browser  │────▶│   wifi-proxy    │────▶│   ESP32 Robot   │
│  localhost:8080 │     │   (USB WiFi)    │     │   (192.168.x.x) │
└─────────────────┘     └─────────────────┘     └─────────────────┘
                              │
                              │ Proxies:
                              │ - /control → robot commands
                              │ - /stream  → camera feed
```

## License

MIT
