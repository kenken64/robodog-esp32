//! Main entry point for the wifi-proxy CLI application.
//!
//! This application provides a command-line interface for managing a secondary USB WiFi
//! adapter to connect to an ESP32 robot dog's access point while maintaining the primary
//! network connection. It supports scanning for networks, connecting, and proxying
//! requests to the robot's web interface.

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use wifi_proxy::{
    config::{self, Config, NetworkConfig},
    connection, interface, scan, server,
};

/// Command-line interface structure for the wifi-proxy application.
/// Uses the `clap` crate for argument parsing with derive macros.
#[derive(Parser)]
#[command(name = "wifi-proxy")]
#[command(about = "Connect a secondary USB WiFi adapter to a different access point")]
#[command(version)]
struct Cli {
    /// The subcommand to execute
    #[command(subcommand)]
    command: Commands,
}

/// Enumeration of all available subcommands for the CLI.
/// Each variant represents a distinct operation the user can perform.
#[derive(Subcommand)]
enum Commands {
    /// List all available WiFi interfaces on the system.
    /// Displays interface name, connection state, and whether it's a USB device.
    ListInterfaces,

    /// Scan for available WiFi networks using the specified interface.
    /// Shows SSID, signal strength, and security type for each network found.
    Scan {
        /// Network interface to use for scanning.
        /// If not specified, auto-detects the first USB WiFi interface.
        #[arg(short, long)]
        interface: Option<String>,
    },

    /// Connect to a WiFi network using the specified credentials.
    /// Supports both interactive password input and saved credential retrieval.
    Connect {
        /// SSID (network name) of the WiFi network to connect to.
        /// This is a required positional argument.
        ssid: String,

        /// Password for the WiFi network.
        /// If not provided, attempts to use a previously saved password from config.
        #[arg(short, long)]
        password: Option<String>,

        /// Network interface to use for the connection.
        /// If not specified, auto-detects the first USB WiFi interface.
        #[arg(short, long)]
        interface: Option<String>,

        /// Flag to save the credentials to the config file after successful connection.
        /// Enables quick reconnection without re-entering the password.
        #[arg(short, long)]
        save: bool,
    },

    /// Display the current connection status for an interface.
    /// Shows state, connected network, IP address, and gateway information.
    Status {
        /// Network interface to check status for.
        /// If not specified, auto-detects the first USB WiFi interface.
        #[arg(short, long)]
        interface: Option<String>,
    },

    /// Disconnect the specified interface from its current network.
    /// Terminates the active WiFi connection.
    Disconnect {
        /// Network interface to disconnect.
        /// If not specified, auto-detects the first USB WiFi interface.
        #[arg(short, long)]
        interface: Option<String>,
    },

    /// Fetch the HTML page from the gateway (robot's web interface) and save it locally.
    /// Useful for debugging or capturing the robot's control interface.
    FetchGateway {
        /// File path where the fetched HTML content will be saved.
        /// Defaults to "gateway.html" in the current directory.
        #[arg(short, long, default_value = "gateway.html")]
        output: PathBuf,

        /// Network interface to use for determining the gateway.
        /// If not specified, auto-detects the first USB WiFi interface.
        #[arg(short, long)]
        interface: Option<String>,

        /// Custom URL to fetch instead of the gateway's root page.
        /// Defaults to "http://<gateway>/" if not specified.
        #[arg(short, long)]
        url: Option<String>,
    },

    /// Start a local web server that proxies requests to the robot's gateway.
    /// Allows controlling the robot from localhost while connected via USB WiFi.
    Serve {
        /// TCP port number for the local web server to listen on.
        /// Defaults to 8080 if not specified.
        #[arg(short, long, default_value = "8080")]
        port: u16,

        /// Network interface to use for proxying to the gateway.
        /// If not specified, auto-detects the first USB WiFi interface.
        #[arg(short, long)]
        interface: Option<String>,
    },

    /// Save network credentials to the configuration file without connecting.
    /// Allows pre-configuring networks for later quick connection.
    SaveNetwork {
        /// SSID (network name) of the WiFi network to save.
        ssid: String,

        /// Password for the WiFi network to save.
        #[arg(short, long)]
        password: String,

        /// Preferred interface to use when connecting to this network.
        /// Optional - if not set, the system will auto-detect.
        #[arg(short, long)]
        interface: Option<String>,
    },

    /// Display the current saved configuration.
    /// Shows all saved networks with masked passwords.
    ShowConfig,
}

/// Application entry point with async runtime support via Tokio.
///
/// Parses command-line arguments and dispatches to the appropriate
/// command handler based on the subcommand provided by the user.
///
/// # Returns
/// - `Ok(())` if the command completes successfully
/// - `Err` with an error message if any operation fails
#[tokio::main]
async fn main() -> Result<()> {
    // Parse command-line arguments into the Cli struct
    let cli = Cli::parse();

    // Match on the subcommand and delegate to the appropriate handler
    match cli.command {
        Commands::ListInterfaces => cmd_list_interfaces(),
        Commands::Scan { interface } => cmd_scan(interface.as_deref()),
        Commands::Connect {
            ssid,
            password,
            interface,
            save,
        } => cmd_connect(&ssid, password.as_deref(), interface.as_deref(), save),
        Commands::Status { interface } => cmd_status(interface.as_deref()),
        Commands::Disconnect { interface } => cmd_disconnect(interface.as_deref()),
        Commands::FetchGateway {
            output,
            interface,
            url,
        } => cmd_fetch_gateway(&output, interface.as_deref(), url.as_deref()),
        Commands::Serve { port, interface } => cmd_serve(port, interface.as_deref()).await,
        Commands::SaveNetwork {
            ssid,
            password,
            interface,
        } => cmd_save_network(&ssid, &password, interface.as_deref()),
        Commands::ShowConfig => cmd_show_config(),
    }
}

/// Handler for the `list-interfaces` command.
///
/// Queries the system for all available WiFi interfaces using nmcli,
/// then displays them in a formatted table showing the interface name,
/// current state (connected/disconnected), and whether it's a USB device.
///
/// # Returns
/// - `Ok(())` on success
/// - `Err` if nmcli command fails or output parsing fails
fn cmd_list_interfaces() -> Result<()> {
    // Retrieve all WiFi interfaces from the system
    let interfaces = interface::list_wifi_interfaces()?;

    // Handle case where no WiFi interfaces are available
    if interfaces.is_empty() {
        println!("No WiFi interfaces found.");
        return Ok(());
    }

    // Print table header with column alignment
    println!("{:<16} {:<12} {}", "INTERFACE", "STATE", "TYPE");
    println!("{}", "-".repeat(40));

    // Iterate through each interface and display its details
    for iface in interfaces {
        // Determine human-readable interface type based on USB detection
        let iface_type = if iface.is_usb { "USB" } else { "Built-in" };
        println!("{:<16} {:<12} {}", iface.name, iface.state, iface_type);
    }

    Ok(())
}

/// Handler for the `scan` command.
///
/// Scans for available WiFi networks using the specified interface
/// (or auto-detected USB interface). Triggers a rescan and displays
/// results sorted by signal strength.
///
/// # Arguments
/// * `interface` - Optional interface name; if None, auto-detects USB interface
///
/// # Returns
/// - `Ok(())` on success
/// - `Err` if interface resolution or scanning fails
fn cmd_scan(interface: Option<&str>) -> Result<()> {
    // Resolve the interface to use (specified or auto-detected USB)
    let iface = interface::resolve_interface(interface)?;
    println!("Scanning on interface: {}", iface.name);
    println!();

    // Perform the network scan and display results in a formatted table
    let networks = scan::scan_networks(&iface.name)?;
    scan::display_networks(&networks);

    Ok(())
}

/// Handler for the `connect` command.
///
/// Connects to a WiFi network using the provided or saved credentials.
/// Optionally saves the credentials for future use.
///
/// # Arguments
/// * `ssid` - The network name to connect to
/// * `password` - Optional password; if None, looks up saved credentials
/// * `interface` - Optional interface name; if None, auto-detects USB interface
/// * `save` - If true, saves credentials to config after successful connection
///
/// # Returns
/// - `Ok(())` on successful connection
/// - `Err` if password is missing and not saved, or connection fails
fn cmd_connect(ssid: &str, password: Option<&str>, interface: Option<&str>, save: bool) -> Result<()> {
    // Load existing config or create a new default config
    let mut cfg = Config::load().unwrap_or_default();

    // Resolve the password: use provided password, or look up saved credentials
    let password = match password {
        Some(p) => p.to_string(),
        None => {
            // Attempt to find saved credentials for this SSID
            if let Some(network) = cfg.find_network(ssid) {
                println!("Using saved password for '{}'", ssid);
                network.password.clone()
            } else {
                // No password provided and no saved credentials - cannot proceed
                bail!("No password provided and no saved credentials for '{}'", ssid);
            }
        }
    };

    // Resolve the interface to use for connection
    let iface = interface::resolve_interface(interface)?;
    println!("Connecting to '{}' on interface {}...", ssid, iface.name);

    // Attempt to establish the WiFi connection using nmcli
    connection::connect(&iface.name, ssid, &password)?;
    println!("Connected successfully!");

    // Optionally save credentials for future quick connections
    if save {
        cfg.add_network(NetworkConfig {
            ssid: ssid.to_string(),
            password,
            interface: Some(iface.name.clone()),
        });
        cfg.save()?;
        println!("Credentials saved to config.");
    }

    // Display the connection status after successful connection
    println!();
    let status = connection::status(&iface.name)?;
    connection::display_status(&status);

    Ok(())
}

/// Handler for the `status` command.
///
/// Displays the current connection status for the specified interface,
/// including connection state, connected network name, IP address, and gateway.
///
/// # Arguments
/// * `interface` - Optional interface name; if None, auto-detects USB interface
///
/// # Returns
/// - `Ok(())` on success
/// - `Err` if interface resolution or status query fails
fn cmd_status(interface: Option<&str>) -> Result<()> {
    // Resolve the interface and query its current status
    let iface = interface::resolve_interface(interface)?;
    let status = connection::status(&iface.name)?;
    connection::display_status(&status);

    Ok(())
}

/// Handler for the `disconnect` command.
///
/// Disconnects the specified interface from its current WiFi network.
///
/// # Arguments
/// * `interface` - Optional interface name; if None, auto-detects USB interface
///
/// # Returns
/// - `Ok(())` on successful disconnection
/// - `Err` if interface resolution or disconnection fails
fn cmd_disconnect(interface: Option<&str>) -> Result<()> {
    // Resolve the interface and initiate disconnection
    let iface = interface::resolve_interface(interface)?;
    println!("Disconnecting interface {}...", iface.name);

    connection::disconnect(&iface.name)?;
    println!("Disconnected.");

    Ok(())
}

/// Handler for the `fetch-gateway` command.
///
/// Fetches the HTML content from the gateway's web interface and saves it
/// to a local file. Useful for debugging or capturing the robot's control page.
///
/// # Arguments
/// * `output` - Path where the fetched HTML will be saved
/// * `interface` - Optional interface name; if None, auto-detects USB interface
/// * `url` - Optional custom URL; if None, uses the gateway's root page
///
/// # Returns
/// - `Ok(())` on successful fetch and save
/// - `Err` if no gateway is found or HTTP request fails
fn cmd_fetch_gateway(output: &PathBuf, interface: Option<&str>, url: Option<&str>) -> Result<()> {
    // Resolve interface and get its connection status to find the gateway
    let iface = interface::resolve_interface(interface)?;
    let status = connection::status(&iface.name)?;

    // Extract the gateway IP address from the connection status
    let gateway = status
        .gateway
        .ok_or_else(|| anyhow::anyhow!("No gateway found for interface {}", iface.name))?;

    // Determine the URL to fetch: custom URL or default gateway root
    let fetch_url = match url {
        Some(u) => u.to_string(),
        None => format!("http://{}/", gateway),
    };

    // Perform the HTTP GET request and save the response to file
    println!("Fetching {} ...", fetch_url);
    connection::fetch_gateway(&fetch_url, output)?;
    println!("Saved to {}", output.display());

    Ok(())
}

/// Handler for the `serve` command (async).
///
/// Starts a local web server that proxies requests to the robot's gateway.
/// This allows controlling the robot from localhost:port while the USB WiFi
/// adapter maintains the connection to the robot's access point.
///
/// # Arguments
/// * `port` - TCP port for the local server to listen on
/// * `interface` - Optional interface name; if None, auto-detects USB interface
///
/// # Returns
/// - `Ok(())` when server shuts down gracefully
/// - `Err` if no gateway found or server fails to start
async fn cmd_serve(port: u16, interface: Option<&str>) -> Result<()> {
    // Resolve interface and get the gateway address for proxying
    let iface = interface::resolve_interface(interface)?;
    let status = connection::status(&iface.name)?;

    // Extract gateway IP - required for proxying requests
    let gateway = status
        .gateway
        .ok_or_else(|| anyhow::anyhow!("No gateway found for interface {}", iface.name))?;

    // Configure and start the proxy server
    let config = server::ServerConfig { gateway, port };
    server::run_server(config).await
}

/// Handler for the `save-network` command.
///
/// Saves network credentials to the configuration file without attempting
/// to connect. Useful for pre-configuring networks.
///
/// # Arguments
/// * `ssid` - Network name to save
/// * `password` - Password for the network
/// * `interface` - Optional preferred interface for this network
///
/// # Returns
/// - `Ok(())` on successful save
/// - `Err` if config file cannot be written
fn cmd_save_network(ssid: &str, password: &str, interface: Option<&str>) -> Result<()> {
    // Load existing config or create default
    let mut cfg = Config::load().unwrap_or_default();

    // Add the network configuration (replaces existing entry with same SSID)
    cfg.add_network(NetworkConfig {
        ssid: ssid.to_string(),
        password: password.to_string(),
        interface: interface.map(String::from),
    });

    // Persist the updated configuration to disk
    cfg.save()?;

    // Confirm the save location to the user
    let path = config::config_path()?;
    println!("Saved network '{}' to {}", ssid, path.display());

    Ok(())
}

/// Handler for the `show-config` command.
///
/// Displays the current configuration including all saved networks.
/// Passwords are masked for security when displayed.
///
/// # Returns
/// - `Ok(())` on success
/// - `Err` if config file cannot be read
fn cmd_show_config() -> Result<()> {
    // Get and display the config file path
    let path = config::config_path()?;
    println!("Config file: {}", path.display());
    println!();

    // Load the current configuration
    let cfg = Config::load()?;

    // Display saved networks in a formatted table
    if cfg.networks.is_empty() {
        println!("No saved networks.");
    } else {
        // Print table header
        println!("{:<24} {:<20} {}", "SSID", "INTERFACE", "PASSWORD");
        println!("{}", "-".repeat(60));

        // Print each saved network with masked password
        for network in &cfg.networks {
            let iface = network.interface.as_deref().unwrap_or("-");
            // Mask password with asterisks (max 12 chars for display)
            let masked_pw = "*".repeat(network.password.len().min(12));
            println!("{:<24} {:<20} {}", network.ssid, iface, masked_pw);
        }
    }

    Ok(())
}
