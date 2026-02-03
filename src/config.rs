//! Configuration management module for WiFi Proxy.
//!
//! This module handles persistent storage of network credentials and application
//! settings using TOML format. Configuration is stored in the user's config directory
//! (e.g., `~/.config/wifi-proxy/config.toml` on Linux).
//!
//! # Configuration File Format
//!
//! ```toml
//! default_interface = "wlan1"  # Optional default interface
//!
//! [[networks]]
//! ssid = "RoboDog-AP"
//! password = "secret123"
//! interface = "wlan1"  # Optional preferred interface
//! ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Main configuration structure containing all application settings.
///
/// This struct is serialized to/from TOML format and contains:
/// - A list of saved network configurations with credentials
/// - An optional default interface to use when none is specified
///
/// # Serialization
///
/// Uses serde with TOML format. Default values are used for missing fields
/// to ensure backwards compatibility with older config files.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    /// List of saved WiFi network configurations.
    /// Each entry contains SSID, password, and optional preferred interface.
    #[serde(default)]
    pub networks: Vec<NetworkConfig>,

    /// Optional default interface name to use when no interface is specified.
    /// If None, the system will auto-detect a USB WiFi interface.
    #[serde(default)]
    pub default_interface: Option<String>,
}

/// Configuration for a single saved WiFi network.
///
/// Contains all information needed to connect to a previously
/// configured network without requiring password re-entry.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkConfig {
    /// The SSID (network name) of the WiFi network.
    pub ssid: String,

    /// The password/pre-shared key for the network.
    /// Stored in plaintext - ensure config file has appropriate permissions.
    pub password: String,

    /// Optional preferred interface to use when connecting to this network.
    /// If None, the system will auto-detect or use the default interface.
    #[serde(default)]
    pub interface: Option<String>,
}

impl Config {
    /// Loads configuration from the default config file path.
    ///
    /// If the config file doesn't exist, returns a default (empty) configuration.
    /// This allows the application to work without requiring initial setup.
    ///
    /// # Returns
    /// - `Ok(Config)` with loaded or default configuration
    /// - `Err` if the file exists but cannot be read or parsed
    ///
    /// # Example
    /// ```no_run
    /// use wifi_proxy::config::Config;
    ///
    /// let cfg = Config::load().expect("Failed to load config");
    /// println!("Saved networks: {}", cfg.networks.len());
    /// ```
    pub fn load() -> Result<Self> {
        // Determine the config file path
        let path = config_path()?;

        // Return default config if file doesn't exist yet
        if !path.exists() {
            return Ok(Config::default());
        }

        // Read the file contents as a string
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        // Parse TOML content into Config struct
        toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))
    }

    /// Saves the current configuration to the default config file path.
    ///
    /// Creates the parent directory if it doesn't exist. Overwrites any
    /// existing config file.
    ///
    /// # Returns
    /// - `Ok(())` on successful save
    /// - `Err` if directory creation or file writing fails
    ///
    /// # Example
    /// ```no_run
    /// use wifi_proxy::config::{Config, NetworkConfig};
    ///
    /// let mut cfg = Config::default();
    /// cfg.add_network(NetworkConfig {
    ///     ssid: "MyNetwork".to_string(),
    ///     password: "secret".to_string(),
    ///     interface: None,
    /// });
    /// cfg.save().expect("Failed to save config");
    /// ```
    pub fn save(&self) -> Result<()> {
        // Get the target config file path
        let path = config_path()?;

        // Ensure the parent directory exists (e.g., ~/.config/wifi-proxy/)
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;
        }

        // Serialize config to pretty-printed TOML format
        let content = toml::to_string_pretty(self)
            .context("Failed to serialize config")?;

        // Write the serialized content to the config file
        fs::write(&path, content)
            .with_context(|| format!("Failed to write config file: {}", path.display()))?;

        Ok(())
    }

    /// Finds a saved network configuration by its SSID.
    ///
    /// Performs a linear search through the saved networks to find
    /// one matching the given SSID.
    ///
    /// # Arguments
    /// * `ssid` - The network name to search for
    ///
    /// # Returns
    /// - `Some(&NetworkConfig)` if a matching network is found
    /// - `None` if no network with the given SSID exists
    pub fn find_network(&self, ssid: &str) -> Option<&NetworkConfig> {
        self.networks.iter().find(|n| n.ssid == ssid)
    }

    /// Adds or updates a network configuration.
    ///
    /// If a network with the same SSID already exists, it is removed
    /// before adding the new configuration. This ensures each SSID
    /// has only one entry.
    ///
    /// # Arguments
    /// * `network` - The network configuration to add
    ///
    /// # Note
    /// Call `save()` after this method to persist changes to disk.
    pub fn add_network(&mut self, network: NetworkConfig) {
        // Remove any existing entry with the same SSID to prevent duplicates
        self.networks.retain(|n| n.ssid != network.ssid);
        // Add the new/updated network configuration
        self.networks.push(network);
    }
}

/// Returns the path to the configuration file.
///
/// Uses the platform-specific config directory:
/// - Linux: `~/.config/wifi-proxy/config.toml`
/// - macOS: `~/Library/Application Support/wifi-proxy/config.toml`
/// - Windows: `C:\Users\<user>\AppData\Roaming\wifi-proxy\config.toml`
///
/// # Returns
/// - `Ok(PathBuf)` with the config file path
/// - `Err` if the config directory cannot be determined
pub fn config_path() -> Result<PathBuf> {
    // Get the system's config directory using the 'dirs' crate
    let config_dir = dirs::config_dir()
        .context("Could not determine config directory")?;

    // Return the full path to our config file
    Ok(config_dir.join("wifi-proxy").join("config.toml"))
}
