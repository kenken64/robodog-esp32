use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::error::WifiProxyError;

#[derive(Debug)]
pub struct ConnectionStatus {
    pub interface: String,
    pub state: String,
    pub connection: Option<String>,
    pub ip_address: Option<String>,
    pub gateway: Option<String>,
}

/// Connect to a WiFi network on the specified interface
pub fn connect(interface: &str, ssid: &str, password: &str) -> Result<()> {
    let output = Command::new("nmcli")
        .args([
            "device",
            "wifi",
            "connect",
            ssid,
            "password",
            password,
            "ifname",
            interface,
        ])
        .output()
        .context("Failed to execute nmcli connect")?;

    if !output.status.success() {
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

/// Disconnect the specified interface
pub fn disconnect(interface: &str) -> Result<()> {
    let output = Command::new("nmcli")
        .args(["device", "disconnect", interface])
        .output()
        .context("Failed to execute nmcli disconnect")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WifiProxyError::NmcliExecution(stderr.to_string()).into());
    }

    Ok(())
}

/// Get the connection status for the specified interface
pub fn status(interface: &str) -> Result<ConnectionStatus> {
    let output = Command::new("nmcli")
        .args(["-t", "device", "show", interface])
        .output()
        .context("Failed to execute nmcli device show")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WifiProxyError::NmcliExecution(stderr.to_string()).into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut status = ConnectionStatus {
        interface: interface.to_string(),
        state: "unknown".to_string(),
        connection: None,
        ip_address: None,
        gateway: None,
    };

    for line in stdout.lines() {
        let parts: Vec<&str> = line.splitn(2, ':').collect();
        if parts.len() != 2 {
            continue;
        }

        let key = parts[0];
        let value = parts[1].to_string();

        match key {
            "GENERAL.STATE" => status.state = value,
            "GENERAL.CONNECTION" => {
                if !value.is_empty() && value != "--" {
                    status.connection = Some(value);
                }
            }
            "IP4.ADDRESS[1]" => {
                status.ip_address = Some(value);
            }
            "IP4.GATEWAY" => {
                if !value.is_empty() && value != "--" {
                    status.gateway = Some(value);
                }
            }
            _ => {}
        }
    }

    Ok(status)
}

/// Display connection status in a formatted way
pub fn display_status(status: &ConnectionStatus) {
    println!("Interface: {}", status.interface);
    println!("State:     {}", status.state);

    if let Some(ref conn) = status.connection {
        println!("Connected: {}", conn);
    } else {
        println!("Connected: (none)");
    }

    if let Some(ref ip) = status.ip_address {
        println!("IP:        {}", ip);
    }

    if let Some(ref gw) = status.gateway {
        println!("Gateway:   {}", gw);
    }
}

/// Delete a saved connection profile by name
pub fn delete_connection(name: &str) -> Result<()> {
    let output = Command::new("nmcli")
        .args(["connection", "delete", name])
        .output()
        .context("Failed to execute nmcli connection delete")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WifiProxyError::NmcliExecution(stderr.to_string()).into());
    }

    Ok(())
}

/// Fetch HTML content from a URL and save it to a file
pub fn fetch_gateway(url: &str, output_path: &Path) -> Result<()> {
    let response = ureq::get(url)
        .call()
        .map_err(|e| WifiProxyError::FetchFailed(e.to_string()))?;

    let content = response
        .into_string()
        .map_err(|e| WifiProxyError::FetchFailed(e.to_string()))?;

    fs::write(output_path, &content).context("Failed to write output file")?;

    Ok(())
}
