//! WiFi connection management module.
//!
//! This module provides functionality for managing WiFi connections using
//! NetworkManager's `nmcli` command-line tool. It supports connecting to
//! networks, disconnecting, querying connection status, and fetching content
//! from the gateway.
//!
//! # Requirements
//!
//! - NetworkManager must be installed and running
//! - The `nmcli` command must be available in PATH
//! - User must have permission to manage network connections
//!
//! # Example
//!
//! ```no_run
//! use wifi_proxy::connection::{connect, status, disconnect};
//!
//! // Connect to a network
//! connect("wlan1", "MyNetwork", "password123").expect("Connect failed");
//!
//! // Check status
//! let s = status("wlan1").expect("Status failed");
//! println!("IP: {:?}", s.ip_address);
//!
//! // Disconnect
//! disconnect("wlan1").expect("Disconnect failed");
//! ```

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::error::WifiProxyError;

/// Represents the current connection status of a WiFi interface.
///
/// Contains information retrieved from NetworkManager about the interface's
/// state, active connection, IP configuration, and gateway address.
#[derive(Debug)]
pub struct ConnectionStatus {
    /// The name of the network interface (e.g., "wlan1").
    pub interface: String,

    /// The current state of the interface (e.g., "100 (connected)", "30 (disconnected)").
    /// This is the raw state string from nmcli.
    pub state: String,

    /// The name of the active connection profile, if connected.
    /// None if not connected to any network.
    pub connection: Option<String>,

    /// The IPv4 address assigned to the interface (with CIDR notation, e.g., "192.168.4.2/24").
    /// None if no IP address is assigned.
    pub ip_address: Option<String>,

    /// The IPv4 gateway address for the connection.
    /// This is typically the robot's IP address (e.g., "192.168.4.1").
    /// None if no gateway is configured.
    pub gateway: Option<String>,
}

/// Connects to a WiFi network using the specified interface.
///
/// Uses NetworkManager's `nmcli` to establish a WiFi connection. This command
/// will create a new connection profile if one doesn't exist for the SSID,
/// or update an existing one with the new credentials.
///
/// # Arguments
/// * `interface` - The name of the WiFi interface to use (e.g., "wlan1")
/// * `ssid` - The SSID (network name) of the WiFi network
/// * `password` - The WPA/WPA2 password for the network
///
/// # Returns
/// - `Ok(())` if the connection is established successfully
/// - `Err(WifiProxyError::ConnectionFailed)` if the connection attempt fails
///
/// # Command Executed
/// ```bash
/// nmcli device wifi connect <ssid> password <password> ifname <interface>
/// ```
///
/// # Example
/// ```no_run
/// use wifi_proxy::connection::connect;
///
/// connect("wlan1", "RoboDog-AP", "password123").expect("Failed to connect");
/// ```
pub fn connect(interface: &str, ssid: &str, password: &str) -> Result<()> {
    // Execute nmcli command to connect to the WiFi network
    let output = Command::new("nmcli")
        .args([
            "device",     // Device management command
            "wifi",       // WiFi-specific operation
            "connect",    // Connect action
            ssid,         // Target network SSID
            "password",   // Password keyword
            password,     // Network password
            "ifname",     // Interface name keyword
            interface,    // Target interface
        ])
        .output()
        .context("Failed to execute nmcli connect")?;

    // Check if the command succeeded
    if !output.status.success() {
        // Extract error message from stderr (preferred) or stdout
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let error_msg = if stderr.is_empty() {
            stdout.to_string()
        } else {
            stderr.to_string()
        };
        return Err(WifiProxyError::ConnectionFailed(error_msg).into());
    }

    Ok(())
}

/// Disconnects the specified interface from its current network.
///
/// Uses NetworkManager's `nmcli` to disconnect the interface. The connection
/// profile is preserved and can be reconnected later.
///
/// # Arguments
/// * `interface` - The name of the WiFi interface to disconnect (e.g., "wlan1")
///
/// # Returns
/// - `Ok(())` if the disconnection is successful
/// - `Err(WifiProxyError::NmcliExecution)` if the command fails
///
/// # Command Executed
/// ```bash
/// nmcli device disconnect <interface>
/// ```
pub fn disconnect(interface: &str) -> Result<()> {
    // Execute nmcli command to disconnect the interface
    let output = Command::new("nmcli")
        .args(["device", "disconnect", interface])
        .output()
        .context("Failed to execute nmcli disconnect")?;

    // Check for command execution errors
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WifiProxyError::NmcliExecution(stderr.to_string()).into());
    }

    Ok(())
}

/// Retrieves the connection status for the specified interface.
///
/// Queries NetworkManager for detailed information about the interface including
/// its connection state, active network name, IP address, and gateway.
///
/// # Arguments
/// * `interface` - The name of the WiFi interface to query (e.g., "wlan1")
///
/// # Returns
/// - `Ok(ConnectionStatus)` containing the interface's current status
/// - `Err(WifiProxyError::NmcliExecution)` if the command fails
///
/// # Command Executed
/// ```bash
/// nmcli -t device show <interface>
/// ```
///
/// The `-t` flag produces terse (machine-readable) output with colon-separated
/// key:value pairs, one per line.
///
/// # Parsed Fields
/// - `GENERAL.STATE` - Interface state (e.g., "100 (connected)")
/// - `GENERAL.CONNECTION` - Active connection profile name
/// - `IP4.ADDRESS[1]` - Primary IPv4 address with CIDR
/// - `IP4.GATEWAY` - IPv4 gateway address
pub fn status(interface: &str) -> Result<ConnectionStatus> {
    // Execute nmcli to get device information in terse format
    let output = Command::new("nmcli")
        .args(["-t", "device", "show", interface])
        .output()
        .context("Failed to execute nmcli device show")?;

    // Check for command execution errors
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WifiProxyError::NmcliExecution(stderr.to_string()).into());
    }

    // Parse the output and build the status struct
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Initialize status with default values
    let mut status = ConnectionStatus {
        interface: interface.to_string(),
        state: "unknown".to_string(),
        connection: None,
        ip_address: None,
        gateway: None,
    };

    // Parse each line of the terse output (format: KEY:VALUE)
    for line in stdout.lines() {
        // Split on first colon only (value might contain colons)
        let parts: Vec<&str> = line.splitn(2, ':').collect();
        if parts.len() != 2 {
            continue; // Skip malformed lines
        }

        let key = parts[0];
        let value = parts[1].to_string();

        // Extract relevant fields based on the key
        match key {
            // Connection state (e.g., "100 (connected)", "30 (disconnected)")
            "GENERAL.STATE" => status.state = value,

            // Active connection profile name (empty or "--" if not connected)
            "GENERAL.CONNECTION" => {
                if !value.is_empty() && value != "--" {
                    status.connection = Some(value);
                }
            }

            // Primary IPv4 address (e.g., "192.168.4.2/24")
            "IP4.ADDRESS[1]" => {
                status.ip_address = Some(value);
            }

            // IPv4 gateway address (e.g., "192.168.4.1")
            "IP4.GATEWAY" => {
                if !value.is_empty() && value != "--" {
                    status.gateway = Some(value);
                }
            }

            // Ignore other fields
            _ => {}
        }
    }

    Ok(status)
}

/// Displays connection status information in a human-readable format.
///
/// Prints the interface name, connection state, connected network (if any),
/// IP address, and gateway to stdout in a formatted layout.
///
/// # Arguments
/// * `status` - The ConnectionStatus struct to display
///
/// # Output Format
/// ```text
/// Interface: wlan1
/// State:     100 (connected)
/// Connected: RoboDog-AP
/// IP:        192.168.4.2/24
/// Gateway:   192.168.4.1
/// ```
pub fn display_status(status: &ConnectionStatus) {
    // Print interface name
    println!("Interface: {}", status.interface);

    // Print current state
    println!("State:     {}", status.state);

    // Print connected network name or "(none)" if disconnected
    if let Some(ref conn) = status.connection {
        println!("Connected: {}", conn);
    } else {
        println!("Connected: (none)");
    }

    // Print IP address if assigned
    if let Some(ref ip) = status.ip_address {
        println!("IP:        {}", ip);
    }

    // Print gateway address if available
    if let Some(ref gw) = status.gateway {
        println!("Gateway:   {}", gw);
    }
}

/// Deletes a saved connection profile from NetworkManager.
///
/// Removes the connection profile by name. This does not disconnect an active
/// connection but removes the saved credentials and settings.
///
/// # Arguments
/// * `name` - The name of the connection profile to delete
///
/// # Returns
/// - `Ok(())` if the deletion is successful
/// - `Err(WifiProxyError::NmcliExecution)` if the command fails
///
/// # Command Executed
/// ```bash
/// nmcli connection delete <name>
/// ```
///
/// # Note
/// This function is currently not used by the CLI but is available for
/// programmatic use.
pub fn delete_connection(name: &str) -> Result<()> {
    // Execute nmcli command to delete the connection profile
    let output = Command::new("nmcli")
        .args(["connection", "delete", name])
        .output()
        .context("Failed to execute nmcli connection delete")?;

    // Check for command execution errors
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WifiProxyError::NmcliExecution(stderr.to_string()).into());
    }

    Ok(())
}

/// Fetches content from a URL and saves it to a file.
///
/// Performs an HTTP GET request to the specified URL (typically the robot's
/// gateway web interface) and writes the response body to the output file.
///
/// # Arguments
/// * `url` - The URL to fetch (e.g., "http://192.168.4.1/")
/// * `output_path` - The file path where the content will be saved
///
/// # Returns
/// - `Ok(())` if the fetch and save are successful
/// - `Err(WifiProxyError::FetchFailed)` if the HTTP request fails
/// - `Err` if writing the file fails
///
/// # Example
/// ```no_run
/// use std::path::Path;
/// use wifi_proxy::connection::fetch_gateway;
///
/// fetch_gateway("http://192.168.4.1/", Path::new("gateway.html"))
///     .expect("Failed to fetch gateway");
/// ```
pub fn fetch_gateway(url: &str, output_path: &Path) -> Result<()> {
    // Perform HTTP GET request using ureq (blocking HTTP client)
    let response = ureq::get(url)
        .call()
        .map_err(|e| WifiProxyError::FetchFailed(e.to_string()))?;

    // Read the response body as a string
    let content = response
        .into_string()
        .map_err(|e| WifiProxyError::FetchFailed(e.to_string()))?;

    // Write the content to the output file
    fs::write(output_path, &content).context("Failed to write output file")?;

    Ok(())
}
