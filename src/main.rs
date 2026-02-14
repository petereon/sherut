use axum::{
    extract::{Extension, MatchedPath, Path, Query},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::any,
    Router,
};
use clap::{Parser, ValueEnum};
use regex::Regex;
use serde_json::json;
use std::{collections::HashMap, env, net::SocketAddr, sync::Arc};
use tokio::process::Command;
use tracing::{debug, error, info, warn, Level};
use tracing_subscriber::FmtSubscriber;

// --- CLI Configuration ---

#[derive(Clone, Debug, ValueEnum)]
enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Clone, Debug, ValueEnum, PartialEq)]
enum ShellType {
    Bash,
    Zsh,
    Fish,
    Sh,
}

impl ShellType {
    fn executable(&self) -> &'static str {
        match self {
            ShellType::Bash => "bash",
            ShellType::Zsh => "zsh",
            ShellType::Fish => "fish",
            ShellType::Sh => "sh",
        }
    }

    fn supports_assoc_arrays(&self) -> bool {
        matches!(self, ShellType::Bash | ShellType::Zsh)
    }
}

#[derive(Clone, Debug, ValueEnum, PartialEq)]
enum HeaderFormat {
    /// Associative array (for bash/zsh)
    Assoc,
    /// JSON string in HEADERS_JSON env var
    Json,
}

/// Detect system default shell from $SHELL environment variable
fn detect_default_shell() -> ShellType {
    if let Ok(shell_path) = env::var("SHELL") {
        let shell_name = shell_path.rsplit('/').next().unwrap_or("");
        match shell_name {
            "bash" => ShellType::Bash,
            "zsh" => ShellType::Zsh,
            "fish" => ShellType::Fish,
            "sh" => ShellType::Sh,
            _ => {
                warn!("Unknown shell '{}', defaulting to bash", shell_name);
                ShellType::Bash
            }
        }
    } else {
        warn!("$SHELL not set, defaulting to bash");
        ShellType::Bash
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about = "Turn any shell command into an API")]
struct Args {
    #[arg(long, default_value_t = 8080)]
    port: u16,

    #[arg(long, value_enum, default_value_t = LogLevel::Info)]
    log_level: LogLevel,

    /// Shell to use for executing commands (auto-detected from $SHELL if not set)
    #[arg(long, value_enum)]
    shell: Option<ShellType>,

    /// Format for passing headers to commands
    /// 'assoc' uses associative arrays (bash/zsh only)
    /// 'json' exports HEADERS_JSON environment variable
    #[arg(long, value_enum)]
    header_format: Option<HeaderFormat>,

    /// Format for passing query string parameters to commands
    /// 'assoc' uses associative arrays (bash/zsh only)
    /// 'json' exports QUERY_JSON environment variable
    #[arg(long, value_enum)]
    query_format: Option<HeaderFormat>,

    #[arg(long = "route", value_names = ["PATH", "COMMAND"], num_args = 2)]
    routes: Vec<String>,
}

// --- Application State ---
#[derive(Clone)]
struct AppState {
    commands: HashMap<String, String>,
    shell: ShellType,
    header_format: HeaderFormat,
    query_format: HeaderFormat,
}

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
    let mut command_map = HashMap::new();
    let route_regex = Regex::new(r":([a-zA-Z0-9_]+)").expect("Invalid regex");

    if args.routes.is_empty() {
        warn!("No routes defined via CLI.");
    }

    for chunk in args.routes.chunks(2) {
        if let [raw_path, cmd] = chunk {
            if cmd.trim().is_empty() {
                error!("Command for route '{}' is empty. Exiting.", raw_path);
                std::process::exit(1);
            }

            // Convert /user/:id to /user/{id} for Axum compatibility
            let normalized_path = route_regex.replace_all(raw_path, "{$1}").to_string();

            command_map.insert(normalized_path.clone(), cmd.clone());
            info!("Registered route: {} -> `{}`", raw_path, cmd);
        }
    }

    let shared_state = Arc::new(AppState {
        commands: command_map.clone(),
        shell,
        header_format,
        query_format,
    });

    // 3. Build Router
    let mut app: Router = Router::new();

    for (path, _) in &command_map {
        app = app.route(path, any(handler));
    }

    // Attach state as an Extension layer
    let app = app
        .layer(Extension(shared_state))
        .fallback(fallback_handler);

    // 4. Start Server
    let addr = SocketAddr::from(([0, 0, 0, 0], args.port));
    info!("🚀 Server running on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    if let Err(e) = axum::serve(listener, app).await {
        error!("Server failed to start: {}", e);
    }
}

// --- Handler ---
// CHANGED: Return type is strictly `Response` to allow dynamic headers.
async fn handler(
    Extension(state): Extension<Arc<AppState>>,
    matched_path: MatchedPath,
    Path(params): Path<HashMap<String, String>>,
    Query(query_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let route_pattern = matched_path.as_str();

    debug!("Handling request for: {}", route_pattern);

    let command_template = match state.commands.get(route_pattern) {
        Some(cmd) => cmd,
        None => {
            error!("Route config missing for pattern: {}", route_pattern);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Config Error".to_string(),
            ).into_response();
        }
    };

    // Replace :param placeholders in command with actual values
    let mut command_with_params = command_template.clone();
    for (key, value) in &params {
        // Escape single quotes in the value for shell safety
        let safe_value = value.replace("'", "'\\''");
        command_with_params = command_with_params.replace(&format!(":{}", key), &safe_value);
    }

    // Collect headers into a map
    let mut headers_map: HashMap<String, String> = HashMap::new();
    for (key, value) in headers.iter() {
        if let Ok(val_str) = value.to_str() {
            headers_map.insert(key.to_string(), val_str.to_string());
        }
    }

    // Build the shell script based on shell type and header format
    let shell_script = build_shell_script(
        &state.shell,
        &state.header_format,
        &headers_map,
        &state.query_format,
        &query_params,
        &command_with_params,
    );

    // Build command with environment inheritance
    let mut cmd = Command::new(state.shell.executable());
    cmd.arg("-c").arg(&shell_script);

    // For JSON header format, also set as environment variable
    if state.header_format == HeaderFormat::Json {
        let headers_json = json!(headers_map).to_string();
        cmd.env("HEADERS_JSON", &headers_json);
    }

    // For JSON query format, also set as environment variable
    if state.query_format == HeaderFormat::Json {
        let query_json = json!(query_params).to_string();
        cmd.env("QUERY_JSON", &query_json);
    }

    let output = cmd.output().await;

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();

            if !out.status.success() {
                warn!("Command failed. Stderr: {}", stderr);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Error:\n{}", stderr),
                ).into_response();
            }

            // --- MAGIC PREFIX PARSING START ---
            let mut builder = Response::builder().status(StatusCode::OK);
            let mut body_accum = String::new();
            let mut content_type_set = false;

            for line in stdout.lines() {
                if let Some(val) = line.strip_prefix("@header:") {
                    // Syntax: @header: Content-Type: application/json
                    if let Some((k, v)) = val.split_once(':') {
                        let header_name = k.trim().to_lowercase();
                        if header_name == "content-type" {
                            content_type_set = true;
                        }
                        builder = builder.header(k.trim(), v.trim());
                        debug!("Set Header: {} -> {}", k.trim(), v.trim());
                    }
                } else if let Some(val) = line.strip_prefix("@status:") {
                    // Syntax: @status: 404
                    if let Ok(code) = val.trim().parse::<u16>() {
                        if let Ok(status_code) = StatusCode::from_u16(code) {
                            builder = builder.status(status_code);
                            debug!("Set Status: {}", status_code);
                        }
                    }
                } else {
                    // Normal content
                    body_accum.push_str(line);
                    body_accum.push('\n');
                }
            }

            // Auto-detect Content-Type if not explicitly set
            if !content_type_set {
                let detected = detect_content_type(&body_accum);
                builder = builder.header("Content-Type", detected);
                debug!("Auto-detected Content-Type: {}", detected);
            }

            // Return the built response
            builder.body(body_accum).unwrap().into_response()
            // --- MAGIC PREFIX PARSING END ---
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Build the shell script with headers and query params in the appropriate format
fn build_shell_script(
    shell: &ShellType,
    header_format: &HeaderFormat,
    headers: &HashMap<String, String>,
    query_format: &HeaderFormat,
    query_params: &HashMap<String, String>,
    command: &str,
) -> String {
    let mut prefix = String::new();

    // Build HEADERS
    if *header_format == HeaderFormat::Assoc {
        let mut header_defs = String::new();
        for (key, value) in headers {
            let safe_val = value.replace("'", "'\\''");
            header_defs.push_str(&format!("[{}]='{}' ", key, safe_val));
        }

        match shell {
            ShellType::Bash => {
                prefix.push_str(&format!("declare -A HEADERS=({}); ", header_defs));
            }
            ShellType::Zsh => {
                prefix.push_str(&format!("typeset -A HEADERS; HEADERS=({}); ", header_defs));
            }
            _ => {}
        }
    }

    // Build QUERY
    if *query_format == HeaderFormat::Assoc {
        let mut query_defs = String::new();
        for (key, value) in query_params {
            let safe_val = value.replace("'", "'\\''");
            query_defs.push_str(&format!("[{}]='{}' ", key, safe_val));
        }

        match shell {
            ShellType::Bash => {
                prefix.push_str(&format!("declare -A QUERY=({}); ", query_defs));
            }
            ShellType::Zsh => {
                prefix.push_str(&format!("typeset -A QUERY; QUERY=({}); ", query_defs));
            }
            _ => {}
        }
    }

    format!("{}{}", prefix, command)
}

/// Auto-detect content type based on body content
fn detect_content_type(body: &str) -> &'static str {
    let trimmed = body.trim();
    
    // Check for JSON: starts with { or [
    if (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
    {
        // Verify it's valid JSON
        if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
            return "application/json";
        }
    }
    
    // Check for XML: starts with < and looks like XML
    if trimmed.starts_with('<') {
        // Check for XML declaration or common XML patterns
        if trimmed.starts_with("<?xml")
            || trimmed.starts_with("<!DOCTYPE")
            || (trimmed.starts_with('<') && trimmed.ends_with('>') && trimmed.contains("</"))
        {
            return "application/xml";
        }
        // Could be HTML
        let lower = trimmed.to_lowercase();
        if lower.starts_with("<!doctype html") || lower.starts_with("<html") {
            return "text/html";
        }
    }
    
    // Default to plain text
    "text/plain"
}

async fn fallback_handler() -> (StatusCode, String) {
    (StatusCode::NOT_FOUND, "Route not found".to_string())
}