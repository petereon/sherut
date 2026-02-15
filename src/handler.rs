use axum::{
    body::Bytes,
    extract::{Extension, MatchedPath, Path, Query},
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
};
use serde_json::json;
use std::{collections::HashMap, process::Stdio, sync::Arc};
use tokio::{io::AsyncWriteExt, process::Command};
use tracing::{debug, error, warn};

use crate::shell::{build_shell_script, HeaderFormat};
use crate::state::AppState;

pub async fn handler(
    Extension(state): Extension<Arc<AppState>>,
    method: Method,
    matched_path: MatchedPath,
    Path(params): Path<HashMap<String, String>>,
    Query(query_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let route_pattern = matched_path.as_str();
    let method_str = method.as_str();

    debug!(
        "Handling {} request for: {} (body: {} bytes)",
        method_str,
        route_pattern,
        body.len()
    );

    // Try method-specific key first, then fall back to ANY
    let method_key = format!("{} {}", method_str, route_pattern);
    let any_key = format!("ANY {}", route_pattern);

    let command_template = state
        .commands
        .get(&method_key)
        .or_else(|| state.commands.get(&any_key));

    let command_template = match command_template {
        Some(cmd) => cmd,
        None => {
            error!(
                "Route config missing for: {} {}",
                method_str, route_pattern
            );
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Config Error".to_string(),
            )
                .into_response();
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
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

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

    // Spawn process and write body to stdin
    let child = cmd.spawn();

    let output = match child {
        Ok(mut child) => {
            // Write request body to stdin
            if let Some(mut stdin) = child.stdin.take() {
                if let Err(e) = stdin.write_all(&body).await {
                    warn!("Failed to write to stdin: {}", e);
                }
                drop(stdin); // Close stdin to signal EOF
            }
            child.wait_with_output().await
        }
        Err(e) => Err(e),
    };

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();

            if !out.status.success() {
                warn!("Command failed. Stderr: {}", stderr);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Error:\n{}", stderr),
                )
                    .into_response();
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

    // Check for HTML/XML: starts with <
    if trimmed.starts_with('<') {
        let lower = trimmed.to_lowercase();
        
        // Check for HTML first (more specific)
        if lower.starts_with("<!doctype html") || lower.starts_with("<html") {
            return "text/html";
        }
        
        // Check for XML declaration or common XML patterns
        if trimmed.starts_with("<?xml")
            || trimmed.starts_with("<!DOCTYPE")
            || (trimmed.starts_with('<') && trimmed.ends_with('>') && trimmed.contains("</"))
        {
            return "application/xml";
        }
    }

    // Default to plain text
    "text/plain"
}

pub async fn fallback_handler() -> (StatusCode, String) {
    (StatusCode::NOT_FOUND, "Route not found".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_content_type_json_object() {
        let body = r#"{"name": "test", "value": 123}"#;
        assert_eq!(detect_content_type(body), "application/json");
    }

    #[test]
    fn test_detect_content_type_json_array() {
        let body = r#"[1, 2, 3]"#;
        assert_eq!(detect_content_type(body), "application/json");
    }

    #[test]
    fn test_detect_content_type_json_with_whitespace() {
        let body = r#"
            {
                "name": "test"
            }
        "#;
        assert_eq!(detect_content_type(body), "application/json");
    }

    #[test]
    fn test_detect_content_type_invalid_json() {
        let body = r#"{not valid json}"#;
        assert_eq!(detect_content_type(body), "text/plain");
    }

    #[test]
    fn test_detect_content_type_xml_declaration() {
        let body = r#"<?xml version="1.0"?><root></root>"#;
        assert_eq!(detect_content_type(body), "application/xml");
    }

    #[test]
    fn test_detect_content_type_xml_doctype() {
        let body = r#"<!DOCTYPE note><note></note>"#;
        assert_eq!(detect_content_type(body), "application/xml");
    }

    #[test]
    fn test_detect_content_type_xml_tags() {
        let body = r#"<root><child>value</child></root>"#;
        assert_eq!(detect_content_type(body), "application/xml");
    }

    #[test]
    fn test_detect_content_type_html_doctype() {
        let body = r#"<!DOCTYPE html><html><body></body></html>"#;
        assert_eq!(detect_content_type(body), "text/html");
    }

    #[test]
    fn test_detect_content_type_html_tag() {
        let body = r#"<html><head></head><body>Hello</body></html>"#;
        assert_eq!(detect_content_type(body), "text/html");
    }

    #[test]
    fn test_detect_content_type_plain_text() {
        let body = "Hello, World!";
        assert_eq!(detect_content_type(body), "text/plain");
    }

    #[test]
    fn test_detect_content_type_empty() {
        let body = "";
        assert_eq!(detect_content_type(body), "text/plain");
    }

    #[test]
    fn test_detect_content_type_whitespace_only() {
        let body = "   \n\t  ";
        assert_eq!(detect_content_type(body), "text/plain");
    }

    #[test]
    fn test_detect_content_type_nested_json() {
        let body = r#"{"users": [{"id": 1, "name": "Alice"}, {"id": 2, "name": "Bob"}]}"#;
        assert_eq!(detect_content_type(body), "application/json");
    }

    #[tokio::test]
    async fn test_fallback_handler() {
        let (status, body) = fallback_handler().await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body, "Route not found");
    }
}
