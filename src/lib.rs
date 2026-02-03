//! WiFi Proxy Library for ESP32 Robot Dog Control
//!
//! This library provides functionality to manage a secondary USB WiFi adapter
//! for connecting to an ESP32 robot dog's access point while maintaining the
//! primary network connection on the built-in WiFi interface.
//!
//! # Modules
//!
//! - [`config`] - Configuration management for saved networks and settings
//! - [`connection`] - WiFi connection management (connect, disconnect, status)
//! - [`error`] - Custom error types for the library
//! - [`interface`] - WiFi interface discovery and management
//! - [`scan`] - WiFi network scanning functionality
//! - [`server`] - HTTP proxy server for robot control interface
//!
//! # Example Usage
//!
//! ```no_run
//! use wifi_proxy::{find_usb_wifi_interface, connect, status};
//!
//! // Find the USB WiFi interface
//! let iface = find_usb_wifi_interface().expect("No USB WiFi found");
//!
//! // Connect to the robot's access point
//! connect(&iface.name, "RoboDog-AP", "password123").expect("Connection failed");
//!
//! // Check connection status
//! let conn_status = status(&iface.name).expect("Status query failed");
//! println!("Gateway: {:?}", conn_status.gateway);
//! ```

/// Configuration module for managing saved networks and application settings.
/// Handles reading/writing TOML config files and credential storage.
pub mod config;

/// Connection module for WiFi network management.
/// Provides functions to connect, disconnect, check status, and fetch gateway content.
pub mod connection;

/// Error module defining custom error types for the library.
/// Uses `thiserror` for ergonomic error handling.
pub mod error;

/// Interface module for WiFi adapter discovery and management.
/// Handles listing interfaces, detecting USB adapters, and interface resolution.
pub mod interface;

/// Scan module for discovering available WiFi networks.
/// Triggers rescans and parses network information from nmcli output.
pub mod scan;

/// Server module providing an HTTP proxy for the robot's control interface.
/// Uses Axum to serve a web interface that proxies requests to the ESP32 gateway.
pub mod server;

// Re-export commonly used items from connection module for convenient access
pub use connection::{connect, disconnect, fetch_gateway, status, ConnectionStatus};

// Re-export the main error type for library users
pub use error::WifiProxyError;

// Re-export interface-related items for discovering and managing WiFi adapters
pub use interface::{
    find_usb_wifi_interface, get_interface, list_wifi_interfaces, resolve_interface, WifiInterface,
};

// Re-export scan-related items for network discovery
pub use scan::{scan_networks, Network};
