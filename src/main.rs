use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use wifi_proxy::{
    config::{self, Config, NetworkConfig},
    connection, interface, scan, server,
};

#[derive(Parser)]
#[command(name = "wifi-proxy")]
#[command(about = "Connect a secondary USB WiFi adapter to a different access point")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List available WiFi interfaces
    ListInterfaces,

    /// Scan for WiFi networks
    Scan {
        /// Interface to use (defaults to auto-detected USB interface)
        #[arg(short, long)]
        interface: Option<String>,
    },

    /// Connect to a WiFi network
    Connect {
        /// SSID of the network to connect to
        ssid: String,

        /// Password for the network (uses saved password if not provided)
        #[arg(short, long)]
        password: Option<String>,

        /// Interface to use (defaults to auto-detected USB interface)
        #[arg(short, long)]
        interface: Option<String>,

        /// Save credentials to config file
        #[arg(short, long)]
        save: bool,
    },

    /// Show connection status
    Status {
        /// Interface to check (defaults to auto-detected USB interface)
        #[arg(short, long)]
        interface: Option<String>,
    },

    /// Disconnect from the current network
    Disconnect {
        /// Interface to disconnect (defaults to auto-detected USB interface)
        #[arg(short, long)]
        interface: Option<String>,
    },

    /// Fetch gateway HTML page and save to file
    FetchGateway {
        /// Output file path
        #[arg(short, long, default_value = "gateway.html")]
        output: PathBuf,

        /// Interface to use (defaults to auto-detected USB interface)
        #[arg(short, long)]
        interface: Option<String>,

        /// Custom URL to fetch (defaults to http://<gateway>/)
        #[arg(short, long)]
        url: Option<String>,
    },

    /// Start web server that proxies requests to the gateway
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "8080")]
        port: u16,

        /// Interface to use (defaults to auto-detected USB interface)
        #[arg(short, long)]
        interface: Option<String>,
    },

    /// Save network credentials to config file
    SaveNetwork {
        /// SSID of the network
        ssid: String,

        /// Password for the network
        #[arg(short, long)]
        password: String,

        /// Preferred interface for this network
        #[arg(short, long)]
        interface: Option<String>,
    },

    /// Show saved configuration
    ShowConfig,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

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

fn cmd_list_interfaces() -> Result<()> {
    let interfaces = interface::list_wifi_interfaces()?;

    if interfaces.is_empty() {
        println!("No WiFi interfaces found.");
        return Ok(());
    }

    println!("{:<16} {:<12} {}", "INTERFACE", "STATE", "TYPE");
    println!("{}", "-".repeat(40));

    for iface in interfaces {
        let iface_type = if iface.is_usb { "USB" } else { "Built-in" };
        println!("{:<16} {:<12} {}", iface.name, iface.state, iface_type);
    }

    Ok(())
}

fn cmd_scan(interface: Option<&str>) -> Result<()> {
    let iface = interface::resolve_interface(interface)?;
    println!("Scanning on interface: {}", iface.name);
    println!();

    let networks = scan::scan_networks(&iface.name)?;
    scan::display_networks(&networks);

    Ok(())
}

fn cmd_connect(ssid: &str, password: Option<&str>, interface: Option<&str>, save: bool) -> Result<()> {
    let mut cfg = Config::load().unwrap_or_default();

    // Get password from argument or config
    let password = match password {
        Some(p) => p.to_string(),
        None => {
            if let Some(network) = cfg.find_network(ssid) {
                println!("Using saved password for '{}'", ssid);
                network.password.clone()
            } else {
                bail!("No password provided and no saved credentials for '{}'", ssid);
            }
        }
    };

    let iface = interface::resolve_interface(interface)?;
    println!("Connecting to '{}' on interface {}...", ssid, iface.name);

    connection::connect(&iface.name, ssid, &password)?;
    println!("Connected successfully!");

    // Save credentials if requested
    if save {
        cfg.add_network(NetworkConfig {
            ssid: ssid.to_string(),
            password,
            interface: Some(iface.name.clone()),
        });
        cfg.save()?;
        println!("Credentials saved to config.");
    }

    // Show status after connecting
    println!();
    let status = connection::status(&iface.name)?;
    connection::display_status(&status);

    Ok(())
}

fn cmd_status(interface: Option<&str>) -> Result<()> {
    let iface = interface::resolve_interface(interface)?;
    let status = connection::status(&iface.name)?;
    connection::display_status(&status);

    Ok(())
}

fn cmd_disconnect(interface: Option<&str>) -> Result<()> {
    let iface = interface::resolve_interface(interface)?;
    println!("Disconnecting interface {}...", iface.name);

    connection::disconnect(&iface.name)?;
    println!("Disconnected.");

    Ok(())
}

fn cmd_fetch_gateway(output: &PathBuf, interface: Option<&str>, url: Option<&str>) -> Result<()> {
    let iface = interface::resolve_interface(interface)?;
    let status = connection::status(&iface.name)?;

    let gateway = status
        .gateway
        .ok_or_else(|| anyhow::anyhow!("No gateway found for interface {}", iface.name))?;

    let fetch_url = match url {
        Some(u) => u.to_string(),
        None => format!("http://{}/", gateway),
    };

    println!("Fetching {} ...", fetch_url);
    connection::fetch_gateway(&fetch_url, output)?;
    println!("Saved to {}", output.display());

    Ok(())
}

async fn cmd_serve(port: u16, interface: Option<&str>) -> Result<()> {
    let iface = interface::resolve_interface(interface)?;
    let status = connection::status(&iface.name)?;

    let gateway = status
        .gateway
        .ok_or_else(|| anyhow::anyhow!("No gateway found for interface {}", iface.name))?;

    let config = server::ServerConfig { gateway, port };
    server::run_server(config).await
}

fn cmd_save_network(ssid: &str, password: &str, interface: Option<&str>) -> Result<()> {
    let mut cfg = Config::load().unwrap_or_default();

    cfg.add_network(NetworkConfig {
        ssid: ssid.to_string(),
        password: password.to_string(),
        interface: interface.map(String::from),
    });

    cfg.save()?;

    let path = config::config_path()?;
    println!("Saved network '{}' to {}", ssid, path.display());

    Ok(())
}

fn cmd_show_config() -> Result<()> {
    let path = config::config_path()?;
    println!("Config file: {}", path.display());
    println!();

    let cfg = Config::load()?;

    if cfg.networks.is_empty() {
        println!("No saved networks.");
    } else {
        println!("{:<24} {:<20} {}", "SSID", "INTERFACE", "PASSWORD");
        println!("{}", "-".repeat(60));
        for network in &cfg.networks {
            let iface = network.interface.as_deref().unwrap_or("-");
            let masked_pw = "*".repeat(network.password.len().min(12));
            println!("{:<24} {:<20} {}", network.ssid, iface, masked_pw);
        }
    }

    Ok(())
}
