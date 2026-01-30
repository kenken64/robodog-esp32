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

lazy_static! {
    pub static ref TEMPLATES: Tera = {
        let mut tera = match Tera::new("templates/**/*") {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Template parsing error: {}", e);
                std::process::exit(1);
            }
        };
        tera.autoescape_on(vec![".html"]);
        tera
    };
}

pub struct ServerConfig {
    pub gateway: String,
    pub port: u16,
}

pub async fn run_server(config: ServerConfig) -> anyhow::Result<()> {
    let state = Arc::new(config);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/control", get(control_proxy))
        .route("/stream", get(stream_proxy))
        .layer(cors)
        .with_state(state.clone());

    let addr = format!("0.0.0.0:{}", state.port);
    println!("Starting server at http://localhost:{}", state.port);
    println!("Proxying to gateway: {}", state.gateway);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn index_handler() -> impl IntoResponse {
    let mut context = Context::new();
    context.insert("stream_url", "/stream");

    match TEMPLATES.render("index.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            eprintln!("Template render error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Template error").into_response()
        }
    }
}

async fn control_proxy(
    State(config): State<Arc<ServerConfig>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let query_string: String = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&");

    let url = format!("http://{}/control?{}", config.gateway, query_string);

    match ureq::get(&url).call() {
        Ok(response) => {
            let body = response.into_string().unwrap_or_default();
            (StatusCode::OK, body)
        }
        Err(e) => (StatusCode::BAD_GATEWAY, format!("Proxy error: {}", e)),
    }
}

async fn stream_proxy(State(config): State<Arc<ServerConfig>>) -> Response {
    let stream_url = format!("http://{}:81/stream", config.gateway);

    let client = reqwest::Client::new();
    match client.get(&stream_url).send().await {
        Ok(response) => {
            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("multipart/x-mixed-replace; boundary=frame")
                .to_string();

            let stream = response.bytes_stream();

            Response::builder()
                .status(StatusCode::OK)
                .header("content-type", content_type)
                .body(Body::from_stream(stream))
                .unwrap()
        }
        Err(e) => Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .body(Body::from(format!("Stream error: {}", e)))
            .unwrap(),
    }
}
