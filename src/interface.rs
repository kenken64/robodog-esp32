use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::error::WifiProxyError;

#[derive(Debug, Clone)]
pub struct WifiInterface {
    pub name: String,
    pub state: String,
    pub is_usb: bool,
}

/// List all WiFi interfaces on the system
pub fn list_wifi_interfaces() -> Result<Vec<WifiInterface>> {
    let output = Command::new("nmcli")
        .args(["-t", "-f", "DEVICE,TYPE,STATE", "device"])
        .output()
        .context("Failed to execute nmcli")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WifiProxyError::NmcliExecution(stderr.to_string()).into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut interfaces = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() >= 3 && parts[1] == "wifi" {
            let name = parts[0].to_string();
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

/// Check if a network interface is USB-based by examining sysfs
fn is_usb_interface(interface_name: &str) -> bool {
    let device_path = format!("/sys/class/net/{}/device", interface_name);
    let path = Path::new(&device_path);

    if !path.exists() {
        return false;
    }

    // Try to resolve the symlink and check if it contains "usb"
    if let Ok(resolved) = fs::read_link(path) {
        if let Some(resolved_str) = resolved.to_str() {
            return resolved_str.contains("usb");
        }
    }

    // Alternative: check uevent file for USB
    let uevent_path = format!("{}/uevent", device_path);
    if let Ok(content) = fs::read_to_string(&uevent_path) {
        if content.contains("usb") {
            return true;
        }
    }

    false
}

/// Find the first USB WiFi interface
pub fn find_usb_wifi_interface() -> Result<WifiInterface> {
    let interfaces = list_wifi_interfaces()?;

    interfaces
        .into_iter()
        .find(|i| i.is_usb)
        .ok_or_else(|| WifiProxyError::NoUsbInterfaceFound.into())
}

/// Get a specific interface by name, verifying it's a WiFi interface
pub fn get_interface(name: &str) -> Result<WifiInterface> {
    let interfaces = list_wifi_interfaces()?;

    interfaces
        .into_iter()
        .find(|i| i.name == name)
        .ok_or_else(|| WifiProxyError::InterfaceNotFound(name.to_string()).into())
}

/// Resolve interface: use provided name or auto-detect USB interface
pub fn resolve_interface(interface: Option<&str>) -> Result<WifiInterface> {
    match interface {
        Some(name) => get_interface(name),
        None => find_usb_wifi_interface(),
    }
}
