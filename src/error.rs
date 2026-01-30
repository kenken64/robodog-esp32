use thiserror::Error;

#[derive(Error, Debug)]
pub enum WifiProxyError {
    #[error("No USB WiFi interface found")]
    NoUsbInterfaceFound,

    #[error("Interface '{0}' not found")]
    InterfaceNotFound(String),

    #[error("Failed to execute nmcli: {0}")]
    NmcliExecution(String),

    #[error("Failed to parse nmcli output: {0}")]
    NmcliParse(String),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Network '{0}' not found")]
    NetworkNotFound(String),

    #[error("Interface '{0}' is not a WiFi device")]
    NotWifiInterface(String),

    #[error("Failed to fetch URL: {0}")]
    FetchFailed(String),
}
