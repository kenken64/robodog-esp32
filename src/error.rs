//! Error types for the WiFi Proxy library.
//!
//! This module defines custom error types using the `thiserror` crate for
//! ergonomic error handling. All errors implement `std::error::Error` and
//! can be easily converted to `anyhow::Error` for propagation.
//!
//! # Error Categories
//!
//! - **Interface Errors**: Problems finding or validating WiFi interfaces
//! - **nmcli Errors**: Failures executing or parsing NetworkManager commands
//! - **Connection Errors**: Problems establishing WiFi connections
//! - **Network Errors**: Issues with HTTP requests to the gateway

use thiserror::Error;

/// Enumeration of all error types that can occur in the WiFi Proxy library.
///
/// Each variant contains contextual information about the specific error
/// condition, which is included in the error message via the `#[error]` attribute.
///
/// # Example
///
/// ```no_run
/// use wifi_proxy::WifiProxyError;
///
/// fn example() -> Result<(), WifiProxyError> {
///     Err(WifiProxyError::NoUsbInterfaceFound)
/// }
/// ```
#[derive(Error, Debug)]
pub enum WifiProxyError {
    /// No USB WiFi interface was detected on the system.
    ///
    /// This error occurs when auto-detection is used but no WiFi interface
    /// connected via USB is found. Check that the USB WiFi adapter is
    /// plugged in and recognized by the system.
    #[error("No USB WiFi interface found")]
    NoUsbInterfaceFound,

    /// The specified interface name was not found on the system.
    ///
    /// Contains the interface name that was requested but not found.
    /// Verify the interface name using `ip link` or `nmcli device`.
    #[error("Interface '{0}' not found")]
    InterfaceNotFound(String),

    /// The nmcli command failed to execute or returned an error.
    ///
    /// Contains the error message from nmcli's stderr output.
    /// This may indicate NetworkManager is not running or the user
    /// lacks permissions to manage network devices.
    #[error("Failed to execute nmcli: {0}")]
    NmcliExecution(String),

    /// Failed to parse the output from an nmcli command.
    ///
    /// Contains a description of what parsing failed.
    /// This may indicate an unexpected nmcli output format,
    /// possibly due to a different NetworkManager version.
    #[error("Failed to parse nmcli output: {0}")]
    NmcliParse(String),

    /// The WiFi connection attempt failed.
    ///
    /// Contains the error message describing why the connection failed.
    /// Common causes include incorrect password, network out of range,
    /// or authentication timeout.
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// The requested network SSID was not found during scanning.
    ///
    /// Contains the SSID that was searched for but not found.
    /// The network may be out of range, hidden, or not broadcasting.
    #[error("Network '{0}' not found")]
    NetworkNotFound(String),

    /// The specified interface exists but is not a WiFi device.
    ///
    /// Contains the interface name that was found but is not a WiFi adapter.
    /// This can occur if an Ethernet or other non-WiFi interface name
    /// is accidentally specified.
    #[error("Interface '{0}' is not a WiFi device")]
    NotWifiInterface(String),

    /// Failed to fetch content from a URL (HTTP request failed).
    ///
    /// Contains the error message describing the fetch failure.
    /// Common causes include network unreachable, connection refused,
    /// timeout, or invalid URL.
    #[error("Failed to fetch URL: {0}")]
    FetchFailed(String),
}
