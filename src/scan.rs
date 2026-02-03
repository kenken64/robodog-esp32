//! WiFi network scanning module.
//!
//! This module provides functionality for scanning nearby WiFi networks
//! and displaying the results. It uses NetworkManager's nmcli tool to
//! trigger scans and retrieve network information.
//!
//! # Scanning Process
//!
//! 1. Triggers a rescan on the specified interface using `nmcli device wifi rescan`
//! 2. Waits briefly for the scan to complete (500ms)
//! 3. Retrieves the list of discovered networks using `nmcli device wifi list`
//! 4. Parses and deduplicates the results
//! 5. Sorts networks by signal strength (strongest first)
//!
//! # Example
//!
//! ```no_run
//! use wifi_proxy::scan::{scan_networks, display_networks};
//!
//! let networks = scan_networks("wlan1").expect("Scan failed");
//! display_networks(&networks);
//! ```

use anyhow::{Context, Result};
use std::process::Command;

use crate::error::WifiProxyError;

/// Represents a discovered WiFi network from a scan.
///
/// Contains the essential information about a network that users need
/// to decide which network to connect to.
#[derive(Debug, Clone)]
pub struct Network {
    /// The SSID (network name) of the WiFi network.
    /// May be empty for hidden networks.
    pub ssid: String,

    /// Signal strength as a percentage (0-100).
    /// Higher values indicate stronger signal and typically better connection quality.
    pub signal: u8,

    /// Security type of the network (e.g., "WPA2", "WPA3", "WEP", "").
    /// Empty string indicates an open network with no encryption.
    pub security: String,
}

/// Scans for WiFi networks visible to the specified interface.
///
/// Triggers a fresh scan, waits for completion, then retrieves and parses
/// the list of discovered networks. Duplicate SSIDs are filtered out
/// (keeping the first occurrence), and results are sorted by signal strength.
///
/// # Arguments
/// * `interface` - The name of the WiFi interface to scan with (e.g., "wlan1")
///
/// # Returns
/// - `Ok(Vec<Network>)` containing discovered networks sorted by signal (strongest first)
/// - `Err(WifiProxyError::NmcliExecution)` if nmcli commands fail
///
/// # Commands Executed
/// ```bash
/// nmcli device wifi rescan ifname <interface>
/// nmcli -t -f SSID,SIGNAL,SECURITY device wifi list ifname <interface>
/// ```
///
/// # Note
/// The rescan command may fail silently if the interface is busy or doesn't
/// support on-demand scanning. The function will still return cached results
/// from the last successful scan in this case.
pub fn scan_networks(interface: &str) -> Result<Vec<Network>> {
    // Step 1: Trigger a rescan on the specified interface
    // This initiates a fresh scan for nearby networks
    // Result is ignored because rescan can fail if already scanning
    let _ = Command::new("nmcli")
        .args(["device", "wifi", "rescan", "ifname", interface])
        .output();

    // Step 2: Wait briefly for the scan to complete
    // 500ms is usually sufficient for most WiFi adapters to complete a scan
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Step 3: Retrieve the list of discovered networks
    let output = Command::new("nmcli")
        .args([
            "-t",               // Terse output (machine-readable)
            "-f",               // Specify fields to output
            "SSID,SIGNAL,SECURITY",  // Fields we want
            "device",           // Device management command
            "wifi",             // WiFi-specific operation
            "list",             // List networks
            "ifname",           // Interface name keyword
            interface,          // Target interface
        ])
        .output()
        .context("Failed to execute nmcli wifi list")?;

    // Check for command execution errors
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WifiProxyError::NmcliExecution(stderr.to_string()).into());
    }

    // Step 4: Parse the output
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut networks = Vec::new();

    // Track seen SSIDs to filter duplicates (same network from multiple APs)
    let mut seen_ssids = std::collections::HashSet::new();

    // Process each line of output (format: SSID:SIGNAL:SECURITY)
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split(':').collect();

        // Need at least 3 parts (SSID, SIGNAL, SECURITY)
        if parts.len() >= 3 {
            let ssid = parts[0].to_string();

            // Skip hidden networks (empty SSID) and duplicates
            if ssid.is_empty() || seen_ssids.contains(&ssid) {
                continue;
            }
            seen_ssids.insert(ssid.clone());

            // Parse signal strength, defaulting to 0 if parsing fails
            let signal: u8 = parts[1].parse().unwrap_or(0);

            // Security field might contain colons (e.g., "WPA1 WPA2:802.1X")
            // so join all remaining parts
            let security = parts[2..].join(":");

            networks.push(Network {
                ssid,
                signal,
                security,
            });
        }
    }

    // Step 5: Sort networks by signal strength in descending order
    // This puts the strongest (best) signals at the top
    networks.sort_by(|a, b| b.signal.cmp(&a.signal));

    Ok(networks)
}

/// Displays a list of networks in a formatted table.
///
/// Prints network information including SSID, signal strength (numeric and visual),
/// and security type in a human-readable table format.
///
/// # Arguments
/// * `networks` - Slice of Network structs to display
///
/// # Output Format
/// ```text
/// SSID                             SIGNAL SECURITY
/// ------------------------------------------------------------
/// MyHomeNetwork                      95% ████ WPA2
/// GuestNetwork                       72% ███░ WPA2
/// OpenCafe                           45% ██░░
/// ```
///
/// # Note
/// Long SSIDs are truncated to fit within the column width with "..." appended.
pub fn display_networks(networks: &[Network]) {
    // Handle empty results
    if networks.is_empty() {
        println!("No networks found.");
        return;
    }

    // Print table header with column alignment
    println!(
        "{:<32} {:>6} {}",
        "SSID", "SIGNAL", "SECURITY"
    );
    println!("{}", "-".repeat(60));

    // Print each network's information
    for network in networks {
        // Convert numeric signal to visual bar representation
        let signal_bar = signal_to_bar(network.signal);

        // Print formatted row with truncated SSID if necessary
        println!(
            "{:<32} {:>3}% {} {}",
            truncate_ssid(&network.ssid, 32),  // SSID truncated to 32 chars
            network.signal,                      // Signal percentage
            signal_bar,                          // Visual signal indicator
            network.security                     // Security type
        );
    }
}

/// Truncates an SSID to fit within a maximum length.
///
/// If the SSID is longer than `max_len`, it is truncated and "..." is appended.
/// This ensures SSIDs don't overflow their column in the display table.
///
/// # Arguments
/// * `ssid` - The SSID string to potentially truncate
/// * `max_len` - Maximum allowed length including the "..." suffix
///
/// # Returns
/// The original SSID if it fits, or a truncated version with "..." appended.
///
/// # Example
/// ```
/// # fn truncate_ssid(ssid: &str, max_len: usize) -> String {
/// #     if ssid.len() > max_len { format!("{}...", &ssid[..max_len - 3]) }
/// #     else { ssid.to_string() }
/// # }
/// assert_eq!(truncate_ssid("Short", 10), "Short");
/// assert_eq!(truncate_ssid("VeryLongNetworkName", 10), "VeryLon...");
/// ```
fn truncate_ssid(ssid: &str, max_len: usize) -> String {
    if ssid.len() > max_len {
        // Truncate and add "..." suffix (accounts for 3 chars)
        format!("{}...", &ssid[..max_len - 3])
    } else {
        ssid.to_string()
    }
}

/// Converts a numeric signal strength to a visual bar representation.
///
/// Uses Unicode block characters to create a 4-segment visual indicator
/// of signal quality, making it easy to quickly assess network strength.
///
/// # Arguments
/// * `signal` - Signal strength as a percentage (0-100)
///
/// # Returns
/// A static string containing a 4-character visual bar:
/// - `████` - Excellent signal (80-100%)
/// - `███░` - Good signal (60-79%)
/// - `██░░` - Fair signal (40-59%)
/// - `█░░░` - Weak signal (20-39%)
/// - `░░░░` - Very weak signal (0-19%)
///
/// # Signal Quality Guidelines
/// - 80%+ : Excellent - Full speed, reliable connection
/// - 60-79%: Good - Most applications work well
/// - 40-59%: Fair - May experience some slowdowns
/// - 20-39%: Weak - Connection may be unreliable
/// - <20%  : Very weak - Connection likely to drop
fn signal_to_bar(signal: u8) -> &'static str {
    match signal {
        80..=100 => "████",  // Excellent signal
        60..=79 => "███░",   // Good signal
        40..=59 => "██░░",   // Fair signal
        20..=39 => "█░░░",   // Weak signal
        _ => "░░░░",         // Very weak or no signal
    }
}
