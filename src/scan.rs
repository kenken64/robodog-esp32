use anyhow::{Context, Result};
use std::process::Command;

use crate::error::WifiProxyError;

#[derive(Debug, Clone)]
pub struct Network {
    pub ssid: String,
    pub signal: u8,
    pub security: String,
}

/// Scan for WiFi networks on the specified interface
pub fn scan_networks(interface: &str) -> Result<Vec<Network>> {
    // First, trigger a rescan
    let _ = Command::new("nmcli")
        .args(["device", "wifi", "rescan", "ifname", interface])
        .output();

    // Small delay to allow scan to complete
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Get the list of networks
    let output = Command::new("nmcli")
        .args([
            "-t",
            "-f",
            "SSID,SIGNAL,SECURITY",
            "device",
            "wifi",
            "list",
            "ifname",
            interface,
        ])
        .output()
        .context("Failed to execute nmcli wifi list")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WifiProxyError::NmcliExecution(stderr.to_string()).into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut networks = Vec::new();
    let mut seen_ssids = std::collections::HashSet::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() >= 3 {
            let ssid = parts[0].to_string();

            // Skip empty SSIDs and duplicates
            if ssid.is_empty() || seen_ssids.contains(&ssid) {
                continue;
            }
            seen_ssids.insert(ssid.clone());

            let signal: u8 = parts[1].parse().unwrap_or(0);
            let security = parts[2..].join(":"); // Security might contain colons

            networks.push(Network {
                ssid,
                signal,
                security,
            });
        }
    }

    // Sort by signal strength (descending)
    networks.sort_by(|a, b| b.signal.cmp(&a.signal));

    Ok(networks)
}

/// Display networks in a formatted table
pub fn display_networks(networks: &[Network]) {
    if networks.is_empty() {
        println!("No networks found.");
        return;
    }

    println!(
        "{:<32} {:>6} {}",
        "SSID", "SIGNAL", "SECURITY"
    );
    println!("{}", "-".repeat(60));

    for network in networks {
        let signal_bar = signal_to_bar(network.signal);
        println!(
            "{:<32} {:>3}% {} {}",
            truncate_ssid(&network.ssid, 32),
            network.signal,
            signal_bar,
            network.security
        );
    }
}

fn truncate_ssid(ssid: &str, max_len: usize) -> String {
    if ssid.len() > max_len {
        format!("{}...", &ssid[..max_len - 3])
    } else {
        ssid.to_string()
    }
}

fn signal_to_bar(signal: u8) -> &'static str {
    match signal {
        80..=100 => "████",
        60..=79 => "███░",
        40..=59 => "██░░",
        20..=39 => "█░░░",
        _ => "░░░░",
    }
}
