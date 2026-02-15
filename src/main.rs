mod cli;
mod handler;
mod routes;
mod shell;
mod state;

use axum::{
    extract::Extension,
    routing::{any, delete, get, patch, post, put},
    Router,
};
use clap::Parser;
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tracing::{error, info, warn, Level};
use tracing_subscriber::FmtSubscriber;

use cli::{Args, LogLevel};
use handler::{fallback_handler, handler};
use routes::parse_routes;
use shell::{detect_default_shell, HeaderFormat};
use state::AppState;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // 1. Initialize Logging
    let trace_level = match args.log_level {
        LogLevel::Error => Level::ERROR,
        LogLevel::Warn => Level::WARN,
        LogLevel::Info => Level::INFO,
        LogLevel::Debug => Level::DEBUG,
        LogLevel::Trace => Level::TRACE,
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(trace_level)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    // 2. Determine shell and header format
    let shell = args.shell.unwrap_or_else(detect_default_shell);
    let header_format = args.header_format.unwrap_or_else(|| {
        if shell.supports_assoc_arrays() {
            HeaderFormat::Assoc
        } else {
            HeaderFormat::Json
        }
    });

    // Warn if using assoc format with a shell that doesn't support it
    if header_format == HeaderFormat::Assoc && !shell.supports_assoc_arrays() {
        warn!(
            "Shell '{}' does not support associative arrays. Consider using --header-format json",
            shell.executable()
        );
    }

    let query_format = args.query_format.unwrap_or_else(|| {
        if shell.supports_assoc_arrays() {
            HeaderFormat::Assoc
        } else {
            HeaderFormat::Json
        }
    });

    if query_format == HeaderFormat::Assoc && !shell.supports_assoc_arrays() {
        warn!(
            "Shell '{}' does not support associative arrays. Consider using --query-format json",
            shell.executable()
        );
    }

    info!("Using shell: {}", shell.executable());
    info!("Header format: {:?}", header_format);
    info!("Query format: {:?}", query_format);

    // 3. Parse and Normalize Routes
    if args.routes.is_empty() {
        warn!("No routes defined via CLI.");
    }

    let routes = parse_routes(&args.routes);

    // Build command map with method+path as key
    let mut command_map = HashMap::new();
    for route in &routes {
        let key = format!("{} {}", route.method, route.path);
        command_map.insert(key, route.command.clone());
    }

    let shared_state = Arc::new(AppState {
        commands: command_map,
        shell,
        header_format,
        query_format,
    });

    // 4. Build Router
    let mut app: Router = Router::new();

    for route in &routes {
        app = match route.method.as_str() {
            "GET" => app.route(&route.path, get(handler)),
            "POST" => app.route(&route.path, post(handler)),
            "PUT" => app.route(&route.path, put(handler)),
            "DELETE" => app.route(&route.path, delete(handler)),
            "PATCH" => app.route(&route.path, patch(handler)),
            _ => app.route(&route.path, any(handler)),
        };
    }

    // Attach state as an Extension layer
    let app = app
        .layer(Extension(shared_state))
        .fallback(fallback_handler);

    // 5. Start Server
    let addr = SocketAddr::from(([0, 0, 0, 0], args.port));
    info!("🚀 Server running on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    if let Err(e) = axum::serve(listener, app).await {
        error!("Server failed to start: {}", e);
    }
}