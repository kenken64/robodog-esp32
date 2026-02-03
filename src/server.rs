//! HTTP proxy server module for the robot control interface.
//!
//! This module implements a web server using the Axum framework that serves
//! as a proxy between a web browser and the ESP32 robot dog's control interface.
//! It allows controlling the robot from localhost while the USB WiFi adapter
//! maintains the connection to the robot's access point.
//!
//! # Architecture
//!
//! ```text
//! Browser (localhost:8080)
//!     │
//!     ▼
//! Proxy Server (this module)
//!     │
//!     ▼ (via USB WiFi)
//! Robot Gateway (192.168.4.1)
//! ```
//!
//! # Endpoints
//!
//! - `GET /` - Serves the control interface HTML page
//! - `GET /control` - Proxies control commands to the robot's `/control` endpoint
//! - `GET /stream` - Proxies the MJPEG video stream from the robot's camera
//!
//! # CORS
//!
//! The server enables permissive CORS to allow web applications from any origin
//! to access the proxy endpoints.

use axum::{
    body::Body,
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::Arc;
use tera::{Context, Tera};
use tower_http::cors::{Any, CorsLayer};

// Initialize template engine at program startup using lazy_static
// This ensures templates are loaded once and reused for all requests
lazy_static! {
    /// Global Tera template engine instance.
    ///
    /// Templates are loaded from the `templates/` directory relative to the
    /// working directory. HTML auto-escaping is enabled for security.
    ///
    /// # Panics
    /// Exits the program if template parsing fails, as the server cannot
    /// function without valid templates.
    pub static ref TEMPLATES: Tera = {
        // Load all template files from the templates directory
        let mut tera = match Tera::new("templates/**/*") {
            Ok(t) => t,
            Err(e) => {
                // Fatal error - cannot serve pages without templates
                eprintln!("Template parsing error: {}", e);
                std::process::exit(1);
            }
        };
        // Enable auto-escaping for HTML files to prevent XSS attacks
        tera.autoescape_on(vec![".html"]);
        tera
    };
}

/// Configuration for the proxy server.
///
/// Contains all settings needed to start and run the server,
/// including the target gateway address and the listening port.
pub struct ServerConfig {
    /// The IP address of the robot's gateway (e.g., "192.168.4.1").
    /// All proxy requests will be forwarded to this address.
    pub gateway: String,

    /// The local TCP port to listen on (e.g., 8080).
    /// The server will be accessible at `http://localhost:<port>/`.
    pub port: u16,
}

/// Starts the HTTP proxy server with the given configuration.
///
/// Sets up the Axum router with all endpoints, configures CORS for cross-origin
/// requests, and begins listening for incoming connections.
///
/// # Arguments
/// * `config` - Server configuration including gateway address and port
///
/// # Returns
/// - `Ok(())` when the server shuts down gracefully
/// - `Err` if the server fails to start or encounters a fatal error
///
/// # Endpoints Registered
/// - `GET /` - Index page handler (serves the control interface)
/// - `GET /control` - Control command proxy (forwards to robot)
/// - `GET /stream` - Video stream proxy (forwards MJPEG from robot camera)
///
/// # Example
/// ```no_run
/// use wifi_proxy::server::{run_server, ServerConfig};
///
/// #[tokio::main]
/// async fn main() {
///     let config = ServerConfig {
///         gateway: "192.168.4.1".to_string(),
///         port: 8080,
///     };
///     run_server(config).await.expect("Server failed");
/// }
/// ```
pub async fn run_server(config: ServerConfig) -> anyhow::Result<()> {
    // Wrap config in Arc for shared ownership across async handlers
    let state = Arc::new(config);

    // Configure CORS to allow requests from any origin
    // This is necessary for web-based control interfaces
    let cors = CorsLayer::new()
        .allow_origin(Any)    // Allow any origin
        .allow_methods(Any)   // Allow all HTTP methods
        .allow_headers(Any);  // Allow all headers

    // Build the Axum router with all routes
    let app = Router::new()
        .route("/", get(index_handler))           // Main control interface page
        .route("/control", get(control_proxy))    // Robot control commands
        .route("/stream", get(stream_proxy))      // Camera video stream
        .layer(cors)                              // Apply CORS middleware
        .with_state(state.clone());               // Share config with handlers

    // Bind to all interfaces (0.0.0.0) on the configured port
    let addr = format!("0.0.0.0:{}", state.port);
    println!("Starting server at http://localhost:{}", state.port);
    println!("Proxying to gateway: {}", state.gateway);

    // Create TCP listener and start serving requests
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Handler for the index page (`GET /`).
///
/// Renders the main control interface HTML page using the Tera template engine.
/// The template receives the stream URL as a context variable for embedding
/// the video stream in the page.
///
/// # Returns
/// - `Html` response with the rendered template on success
/// - `500 Internal Server Error` if template rendering fails
///
/// # Template Context Variables
/// - `stream_url` - URL for the video stream (set to "/stream")
async fn index_handler() -> impl IntoResponse {
    // Create template context with variables needed by the template
    let mut context = Context::new();
    context.insert("stream_url", "/stream");  // Local proxy URL for the stream

    // Render the index.html template with the context
    match TEMPLATES.render("index.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            // Log the error and return a generic error response
            eprintln!("Template render error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Template error").into_response()
        }
    }
}

/// Handler for control commands (`GET /control`).
///
/// Proxies control requests to the robot's gateway. Query parameters from
/// the incoming request are forwarded unchanged to the robot's `/control`
/// endpoint. This allows the web interface to send motor commands, LED
/// controls, and other robot functions.
///
/// # Arguments
/// * `State(config)` - Shared server configuration containing gateway address
/// * `Query(params)` - Query parameters to forward to the robot
///
/// # Returns
/// - `200 OK` with the robot's response body on success
/// - `502 Bad Gateway` if the proxy request to the robot fails
///
/// # Example Request Flow
/// ```text
/// Browser: GET /control?var=speed&val=100
///    │
///    ▼
/// Proxy: GET http://192.168.4.1/control?var=speed&val=100
///    │
///    ▼
/// Robot: Processes command, returns response
/// ```
async fn control_proxy(
    State(config): State<Arc<ServerConfig>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    // Reconstruct the query string from the parsed parameters
    let query_string: String = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&");

    // Build the full URL to the robot's control endpoint
    let url = format!("http://{}/control?{}", config.gateway, query_string);

    // Forward the request to the robot using ureq (blocking HTTP client)
    match ureq::get(&url).call() {
        Ok(response) => {
            // Successfully received response from robot
            let body = response.into_string().unwrap_or_default();
            (StatusCode::OK, body)
        }
        Err(e) => {
            // Failed to reach the robot or request error
            (StatusCode::BAD_GATEWAY, format!("Proxy error: {}", e))
        }
    }
}

/// Handler for video stream proxy (`GET /stream`).
///
/// Proxies the MJPEG video stream from the robot's camera. The ESP32-CAM
/// typically serves the stream on port 81. This handler establishes a
/// streaming connection and forwards the multipart MJPEG data to the client.
///
/// # Arguments
/// * `State(config)` - Shared server configuration containing gateway address
///
/// # Returns
/// - Streaming `Response` with the video data on success
/// - `502 Bad Gateway` if the stream connection fails
///
/// # Stream Format
/// The robot typically sends MJPEG streams using:
/// - Content-Type: `multipart/x-mixed-replace; boundary=frame`
/// - Each frame is a JPEG image separated by the boundary marker
///
/// # Note
/// This uses reqwest (async HTTP client) instead of ureq because streaming
/// requires async support for efficient handling of the continuous data flow.
async fn stream_proxy(State(config): State<Arc<ServerConfig>>) -> Response {
    // Build the stream URL - ESP32-CAM typically serves on port 81
    let stream_url = format!("http://{}:81/stream", config.gateway);

    // Use reqwest async client for streaming support
    let client = reqwest::Client::new();

    match client.get(&stream_url).send().await {
        Ok(response) => {
            // Extract content-type header to preserve MJPEG boundary info
            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("multipart/x-mixed-replace; boundary=frame")
                .to_string();

            // Get the response body as a byte stream
            let stream = response.bytes_stream();

            // Build and return a streaming response that forwards the video data
            Response::builder()
                .status(StatusCode::OK)
                .header("content-type", content_type)
                .body(Body::from_stream(stream))
                .unwrap()
        }
        Err(e) => {
            // Failed to connect to the camera stream
            Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::from(format!("Stream error: {}", e)))
                .unwrap()
        }
    }
}
