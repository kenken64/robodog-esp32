//! WiFi interface discovery and management module.
//!
//! This module provides functionality for discovering WiFi interfaces on the system,
//! determining whether they are USB-based, and resolving interface names for use
//! with other operations.
//!
//! # USB Detection
//!
//! USB interfaces are detected by examining the Linux sysfs filesystem. An interface
//! is considered USB-based if its device path (resolved from `/sys/class/net/<iface>/device`)
//! contains "usb" in the path.
//!
//! # Example
//!
//! ```no_run
//! use wifi_proxy::interface::{list_wifi_interfaces, find_usb_wifi_interface};
//!
//! // List all WiFi interfaces
//! let interfaces = list_wifi_interfaces().expect("Failed to list interfaces");
//! for iface in interfaces {
//!     println!("{}: USB={}", iface.name, iface.is_usb);
//! }
//!
//! // Find the first USB WiFi interface
//! let usb_iface = find_usb_wifi_interface().expect("No USB WiFi found");
//! println!("Using USB interface: {}", usb_iface.name);
//! ```

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::error::WifiProxyError;

/// Represents a WiFi network interface on the system.
///
/// Contains information about the interface's name, current state,
/// and whether it's connected via USB (as opposed to built-in/PCIe).
#[derive(Debug, Clone)]
pub struct WifiInterface {
    /// The interface name as shown by `ip link` (e.g., "wlan0", "wlan1").
    pub name: String,

    /// The current state of the interface as reported by NetworkManager.
    /// Examples: "connected", "disconnected", "unavailable".
    pub state: String,

    /// True if this interface is a USB WiFi adapter, false otherwise.
    /// USB adapters are typically secondary interfaces used for connecting
    /// to the robot while the built-in WiFi maintains the primary connection.
    pub is_usb: bool,
}

/// Lists all WiFi interfaces available on the system.
///
/// Queries NetworkManager using nmcli to get a list of all network devices,
/// then filters to only WiFi interfaces. For each interface, it also checks
/// whether it's a USB device.
///
/// # Returns
/// - `Ok(Vec<WifiInterface>)` containing all discovered WiFi interfaces
/// - `Err(WifiProxyError::NmcliExecution)` if nmcli command fails
///
/// # Command Executed
/// ```bash
/// nmcli -t -f DEVICE,TYPE,STATE device
/// ```
///
/// The `-t` flag produces terse output, and `-f` specifies the fields to display.
/// Output format is `device:type:state` per line.
///
/// # Example
/// ```no_run
/// use wifi_proxy::interface::list_wifi_interfaces;
///
/// let interfaces = list_wifi_interfaces().expect("Failed to list");
/// for iface in interfaces {
///     println!("{}: {} (USB: {})", iface.name, iface.state, iface.is_usb);
/// }
/// ```
pub fn list_wifi_interfaces() -> Result<Vec<WifiInterface>> {
    // Execute nmcli to list all network devices in terse format
    let output = Command::new("nmcli")
        .args(["-t", "-f", "DEVICE,TYPE,STATE", "device"])
        .output()
        .context("Failed to execute nmcli")?;

    // Check for command execution errors
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WifiProxyError::NmcliExecution(stderr.to_string()).into());
    }

    // Parse the output to extract WiFi interfaces
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut interfaces = Vec::new();

    // Process each line (format: DEVICE:TYPE:STATE)
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split(':').collect();

        // Only process lines with enough parts and type == "wifi"
        if parts.len() >= 3 && parts[1] == "wifi" {
            let name = parts[0].to_string();

            // Check if this interface is USB-based
            let is_usb = is_usb_interface(&name);

            interfaces.push(WifiInterface {
                name,
                state: parts[2].to_string(),
                is_usb,
            });
        }
    }

    Ok(interfaces)
}

/// Checks if a network interface is USB-based by examining the Linux sysfs.
///
/// This function uses two methods to detect USB interfaces:
/// 1. Resolving the device symlink and checking if the path contains "usb"
/// 2. Reading the uevent file and checking for USB references
///
/// # Arguments
/// * `interface_name` - The name of the interface to check (e.g., "wlan1")
///
/// # Returns
/// - `true` if the interface is a USB device
/// - `false` if it's built-in, PCIe, or if detection fails
///
/// # Implementation Details
///
/// On Linux, network interfaces have entries in `/sys/class/net/<interface>/`.
/// The `device` symlink points to the actual hardware device in the sysfs tree.
/// USB devices have paths containing "usb" (e.g., `/sys/devices/pci0000:00/.../usb1/...`).
fn is_usb_interface(interface_name: &str) -> bool {
    // Construct the path to the device symlink in sysfs
    let device_path = format!("/sys/class/net/{}/device", interface_name);
    let path = Path::new(&device_path);

    // If the device path doesn't exist, this might be a virtual interface
    if !path.exists() {
        return false;
    }

    // Method 1: Resolve the symlink and check if path contains "usb"
    // This is the most reliable method for detecting USB devices
    if let Ok(resolved) = fs::read_link(path) {
        if let Some(resolved_str) = resolved.to_str() {
            // USB devices have "usb" somewhere in their sysfs path
            return resolved_str.contains("usb");
        }
    }

    // Method 2: Check the uevent file for USB-related entries
    // This is a fallback in case symlink resolution fails
    let uevent_path = format!("{}/uevent", device_path);
    if let Ok(content) = fs::read_to_string(&uevent_path) {
        if content.contains("usb") {
            return true;
        }
    }

    // Default to false if neither method found USB indicators
    false
}

/// Finds the first USB WiFi interface on the system.
///
/// Scans all WiFi interfaces and returns the first one that is identified
/// as a USB device. This is useful for auto-detecting secondary WiFi adapters.
///
/// # Returns
/// - `Ok(WifiInterface)` with the first USB WiFi interface found
/// - `Err(WifiProxyError::NoUsbInterfaceFound)` if no USB WiFi interfaces exist
///
/// # Example
/// ```no_run
/// use wifi_proxy::interface::find_usb_wifi_interface;
///
/// match find_usb_wifi_interface() {
///     Ok(iface) => println!("Found USB WiFi: {}", iface.name),
///     Err(_) => println!("No USB WiFi adapter found"),
/// }
/// ```
pub fn find_usb_wifi_interface() -> Result<WifiInterface> {
    // Get all WiFi interfaces
    let interfaces = list_wifi_interfaces()?;

    // Find the first one marked as USB
    interfaces
        .into_iter()
        .find(|i| i.is_usb)
        .ok_or_else(|| WifiProxyError::NoUsbInterfaceFound.into())
}

/// Gets a specific WiFi interface by name.
///
/// Verifies that the named interface exists and is a WiFi device (not Ethernet
/// or other type).
///
/// # Arguments
/// * `name` - The interface name to look up (e.g., "wlan1")
///
/// # Returns
/// - `Ok(WifiInterface)` if the interface exists and is a WiFi device
/// - `Err(WifiProxyError::InterfaceNotFound)` if no WiFi interface with that name exists
///
/// # Example
/// ```no_run
/// use wifi_proxy::interface::get_interface;
///
/// let iface = get_interface("wlan1").expect("Interface not found");
/// println!("State: {}", iface.state);
/// ```
pub fn get_interface(name: &str) -> Result<WifiInterface> {
    // Get all WiFi interfaces
    let interfaces = list_wifi_interfaces()?;

    // Find the one matching the requested name
    interfaces
        .into_iter()
        .find(|i| i.name == name)
        .ok_or_else(|| WifiProxyError::InterfaceNotFound(name.to_string()).into())
}

/// Resolves which interface to use based on optional user input.
///
/// If an interface name is provided, validates it exists. If no interface
/// is specified, auto-detects the first USB WiFi interface.
///
/// This function is the primary entry point for interface selection in
/// command handlers, providing a consistent interface resolution strategy.
///
/// # Arguments
/// * `interface` - Optional interface name; if None, auto-detects USB interface
///
/// # Returns
/// - `Ok(WifiInterface)` with the resolved interface
/// - `Err` if the specified interface doesn't exist or no USB interface is found
///
/// # Example
/// ```no_run
/// use wifi_proxy::interface::resolve_interface;
///
/// // Use specified interface
/// let iface = resolve_interface(Some("wlan1")).expect("Not found");
///
/// // Auto-detect USB interface
/// let usb_iface = resolve_interface(None).expect("No USB WiFi");
/// ```
pub fn resolve_interface(interface: Option<&str>) -> Result<WifiInterface> {
    match interface {
        // User specified an interface name - validate it exists
        Some(name) => get_interface(name),
        // No interface specified - auto-detect USB WiFi
        None => find_usb_wifi_interface(),
    }
}
