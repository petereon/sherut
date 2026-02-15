use regex::Regex;
use tracing::{error, info};

/// Route entry with method and path
#[derive(Clone, Debug)]
pub struct RouteEntry {
    pub method: String,
    pub path: String,
    pub command: String,
}

/// Parse route specification like "GET /hello/:name" or just "/hello/:name"
pub fn parse_route_spec(spec: &str) -> (String, String) {
    let spec = spec.trim();
    let parts: Vec<&str> = spec.splitn(2, ' ').collect();

    if parts.len() == 2 {
        let method = parts[0].to_uppercase();
        let path = parts[1].to_string();
        // Validate method
        match method.as_str() {
            "GET" | "POST" | "PUT" | "DELETE" | "PATCH" | "HEAD" | "OPTIONS" | "ANY" => {
                (method, path)
            }
            _ => {
                // Assume it's a path starting with something that looks like a method
                ("ANY".to_string(), spec.to_string())
            }
        }
    } else {
        // No method specified, default to ANY
        ("ANY".to_string(), spec.to_string())
    }
}

/// Parse CLI route arguments into RouteEntry structs
pub fn parse_routes(raw_routes: &[String]) -> Vec<RouteEntry> {
    let mut routes: Vec<RouteEntry> = Vec::new();
    let route_regex = Regex::new(r":([a-zA-Z0-9_]+)").expect("Invalid regex");

    for chunk in raw_routes.chunks(2) {
        if let [raw_spec, cmd] = chunk {
            if cmd.trim().is_empty() {
                error!("Command for route '{}' is empty. Exiting.", raw_spec);
                std::process::exit(1);
            }

            let (method, raw_path) = parse_route_spec(raw_spec);

            // Convert /user/:id to /user/{id} for Axum compatibility
            let normalized_path = route_regex.replace_all(&raw_path, "{$1}").to_string();

            routes.push(RouteEntry {
                method: method.clone(),
                path: normalized_path.clone(),
                command: cmd.clone(),
            });
            info!("Registered route: {} {} -> `{}`", method, raw_path, cmd);
        }
    }

    routes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_route_spec_with_method() {
        let (method, path) = parse_route_spec("GET /hello");
        assert_eq!(method, "GET");
        assert_eq!(path, "/hello");
    }

    #[test]
    fn test_parse_route_spec_post() {
        let (method, path) = parse_route_spec("POST /users");
        assert_eq!(method, "POST");
        assert_eq!(path, "/users");
    }

    #[test]
    fn test_parse_route_spec_lowercase_method() {
        let (method, path) = parse_route_spec("get /hello");
        assert_eq!(method, "GET");
        assert_eq!(path, "/hello");
    }

    #[test]
    fn test_parse_route_spec_without_method() {
        let (method, path) = parse_route_spec("/hello/:name");
        assert_eq!(method, "ANY");
        assert_eq!(path, "/hello/:name");
    }

    #[test]
    fn test_parse_route_spec_any_method() {
        let (method, path) = parse_route_spec("ANY /api");
        assert_eq!(method, "ANY");
        assert_eq!(path, "/api");
    }

    #[test]
    fn test_parse_route_spec_invalid_method_becomes_any() {
        let (method, path) = parse_route_spec("INVALID /path");
        assert_eq!(method, "ANY");
        assert_eq!(path, "INVALID /path");
    }

    #[test]
    fn test_parse_route_spec_all_methods() {
        for method in ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"] {
            let spec = format!("{} /test", method);
            let (parsed_method, _) = parse_route_spec(&spec);
            assert_eq!(parsed_method, method);
        }
    }

    #[test]
    fn test_parse_route_spec_trims_whitespace() {
        let (method, path) = parse_route_spec("  GET /hello  ");
        assert_eq!(method, "GET");
        assert_eq!(path, "/hello");
    }

    #[test]
    fn test_parse_routes_normalizes_params() {
        let raw = vec![
            "GET /user/:id".to_string(),
            "echo :id".to_string(),
        ];
        let routes = parse_routes(&raw);

        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].method, "GET");
        assert_eq!(routes[0].path, "/user/{id}");
        assert_eq!(routes[0].command, "echo :id");
    }

    #[test]
    fn test_parse_routes_multiple() {
        let raw = vec![
            "GET /hello".to_string(),
            "echo hello".to_string(),
            "POST /data".to_string(),
            "cat".to_string(),
        ];
        let routes = parse_routes(&raw);

        assert_eq!(routes.len(), 2);
        assert_eq!(routes[0].method, "GET");
        assert_eq!(routes[0].path, "/hello");
        assert_eq!(routes[1].method, "POST");
        assert_eq!(routes[1].path, "/data");
    }

    #[test]
    fn test_parse_routes_multiple_params() {
        let raw = vec![
            "/users/:user_id/posts/:post_id".to_string(),
            "echo :user_id :post_id".to_string(),
        ];
        let routes = parse_routes(&raw);

        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].path, "/users/{user_id}/posts/{post_id}");
    }

    #[test]
    fn test_parse_routes_empty() {
        let raw: Vec<String> = vec![];
        let routes = parse_routes(&raw);
        assert!(routes.is_empty());
    }
}
