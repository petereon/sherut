use clap::ValueEnum;
use std::{collections::HashMap, env};
use tracing::warn;

#[derive(Clone, Debug, ValueEnum, PartialEq)]
pub enum ShellType {
    Bash,
    Zsh,
    Fish,
    Sh,
}

impl ShellType {
    pub fn executable(&self) -> &'static str {
        match self {
            ShellType::Bash => "bash",
            ShellType::Zsh => "zsh",
            ShellType::Fish => "fish",
            ShellType::Sh => "sh",
        }
    }

    pub fn supports_assoc_arrays(&self) -> bool {
        matches!(self, ShellType::Bash | ShellType::Zsh)
    }
}

#[derive(Clone, Debug, ValueEnum, PartialEq)]
pub enum HeaderFormat {
    /// Associative array (for bash/zsh)
    Assoc,
    /// JSON string in HEADERS_JSON env var
    Json,
}

/// Detect system default shell from $SHELL environment variable
pub fn detect_default_shell() -> ShellType {
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

/// Build the shell script with headers and query params in the appropriate format
pub fn build_shell_script(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_executable() {
        assert_eq!(ShellType::Bash.executable(), "bash");
        assert_eq!(ShellType::Zsh.executable(), "zsh");
        assert_eq!(ShellType::Fish.executable(), "fish");
        assert_eq!(ShellType::Sh.executable(), "sh");
    }

    #[test]
    fn test_supports_assoc_arrays() {
        assert!(ShellType::Bash.supports_assoc_arrays());
        assert!(ShellType::Zsh.supports_assoc_arrays());
        assert!(!ShellType::Fish.supports_assoc_arrays());
        assert!(!ShellType::Sh.supports_assoc_arrays());
    }

    #[test]
    fn test_build_shell_script_json_format() {
        let headers = HashMap::new();
        let query = HashMap::new();
        let script = build_shell_script(
            &ShellType::Bash,
            &HeaderFormat::Json,
            &headers,
            &HeaderFormat::Json,
            &query,
            "echo hello",
        );
        assert_eq!(script, "echo hello");
    }

    #[test]
    fn test_build_shell_script_bash_assoc() {
        let mut headers = HashMap::new();
        headers.insert("content-type".to_string(), "application/json".to_string());

        let query = HashMap::new();
        let script = build_shell_script(
            &ShellType::Bash,
            &HeaderFormat::Assoc,
            &headers,
            &HeaderFormat::Json,
            &query,
            "echo hello",
        );

        assert!(script.starts_with("declare -A HEADERS=("));
        assert!(script.contains("[content-type]='application/json'"));
        assert!(script.ends_with("echo hello"));
    }

    #[test]
    fn test_build_shell_script_zsh_assoc() {
        let mut headers = HashMap::new();
        headers.insert("x-api-key".to_string(), "secret".to_string());

        let query = HashMap::new();
        let script = build_shell_script(
            &ShellType::Zsh,
            &HeaderFormat::Assoc,
            &headers,
            &HeaderFormat::Json,
            &query,
            "echo hello",
        );

        assert!(script.starts_with("typeset -A HEADERS; HEADERS=("));
        assert!(script.contains("[x-api-key]='secret'"));
        assert!(script.ends_with("echo hello"));
    }

    #[test]
    fn test_build_shell_script_query_assoc() {
        let headers = HashMap::new();
        let mut query = HashMap::new();
        query.insert("page".to_string(), "1".to_string());
        query.insert("limit".to_string(), "10".to_string());

        let script = build_shell_script(
            &ShellType::Bash,
            &HeaderFormat::Json,
            &headers,
            &HeaderFormat::Assoc,
            &query,
            "echo test",
        );

        assert!(script.contains("declare -A QUERY=("));
        assert!(script.contains("[page]='1'"));
        assert!(script.contains("[limit]='10'"));
    }

    #[test]
    fn test_build_shell_script_escapes_single_quotes() {
        let mut headers = HashMap::new();
        headers.insert("value".to_string(), "it's a test".to_string());

        let query = HashMap::new();
        let script = build_shell_script(
            &ShellType::Bash,
            &HeaderFormat::Assoc,
            &headers,
            &HeaderFormat::Json,
            &query,
            "echo hello",
        );

        assert!(script.contains("it'\\''s a test"));
    }

    #[test]
    fn test_build_shell_script_fish_ignores_assoc() {
        let mut headers = HashMap::new();
        headers.insert("key".to_string(), "value".to_string());

        let query = HashMap::new();
        let script = build_shell_script(
            &ShellType::Fish,
            &HeaderFormat::Assoc,
            &headers,
            &HeaderFormat::Assoc,
            &query,
            "echo hello",
        );

        // Fish doesn't support assoc arrays, so prefix should be empty
        assert_eq!(script, "echo hello");
    }
}
