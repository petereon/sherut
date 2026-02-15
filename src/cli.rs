use clap::{Parser, ValueEnum};

use crate::shell::{HeaderFormat, ShellType};

#[derive(Clone, Debug, ValueEnum)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Parser, Debug)]
#[command(author, version, about = "Turn any shell command into an API")]
pub struct Args {
    #[arg(long, default_value_t = 8080)]
    pub port: u16,

    #[arg(long, value_enum, default_value_t = LogLevel::Info)]
    pub log_level: LogLevel,

    /// Shell to use for executing commands (auto-detected from $SHELL if not set)
    #[arg(long, value_enum)]
    pub shell: Option<ShellType>,

    /// Format for passing headers to commands
    /// 'assoc' uses associative arrays (bash/zsh only)
    /// 'json' exports HEADERS_JSON environment variable
    #[arg(long, value_enum)]
    pub header_format: Option<HeaderFormat>,

    /// Format for passing query string parameters to commands
    /// 'assoc' uses associative arrays (bash/zsh only)
    /// 'json' exports QUERY_JSON environment variable
    #[arg(long, value_enum)]
    pub query_format: Option<HeaderFormat>,

    #[arg(long = "route", value_names = ["PATH", "COMMAND"], num_args = 2)]
    pub routes: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_default_port() {
        let args = Args::parse_from(["sherut"]);
        assert_eq!(args.port, 8080);
    }

    #[test]
    fn test_custom_port() {
        let args = Args::parse_from(["sherut", "--port", "3000"]);
        assert_eq!(args.port, 3000);
    }

    #[test]
    fn test_default_log_level() {
        let args = Args::parse_from(["sherut"]);
        assert!(matches!(args.log_level, LogLevel::Info));
    }

    #[test]
    fn test_log_level_debug() {
        let args = Args::parse_from(["sherut", "--log-level", "debug"]);
        assert!(matches!(args.log_level, LogLevel::Debug));
    }

    #[test]
    fn test_log_level_error() {
        let args = Args::parse_from(["sherut", "--log-level", "error"]);
        assert!(matches!(args.log_level, LogLevel::Error));
    }

    #[test]
    fn test_shell_option() {
        let args = Args::parse_from(["sherut", "--shell", "bash"]);
        assert_eq!(args.shell, Some(ShellType::Bash));
    }

    #[test]
    fn test_shell_zsh() {
        let args = Args::parse_from(["sherut", "--shell", "zsh"]);
        assert_eq!(args.shell, Some(ShellType::Zsh));
    }

    #[test]
    fn test_no_shell_default() {
        let args = Args::parse_from(["sherut"]);
        assert!(args.shell.is_none());
    }

    #[test]
    fn test_header_format_json() {
        let args = Args::parse_from(["sherut", "--header-format", "json"]);
        assert_eq!(args.header_format, Some(HeaderFormat::Json));
    }

    #[test]
    fn test_header_format_assoc() {
        let args = Args::parse_from(["sherut", "--header-format", "assoc"]);
        assert_eq!(args.header_format, Some(HeaderFormat::Assoc));
    }

    #[test]
    fn test_query_format_json() {
        let args = Args::parse_from(["sherut", "--query-format", "json"]);
        assert_eq!(args.query_format, Some(HeaderFormat::Json));
    }

    #[test]
    fn test_single_route() {
        let args = Args::parse_from([
            "sherut",
            "--route", "GET /hello", "echo hello",
        ]);
        assert_eq!(args.routes, vec!["GET /hello", "echo hello"]);
    }

    #[test]
    fn test_multiple_routes() {
        let args = Args::parse_from([
            "sherut",
            "--route", "GET /hello", "echo hello",
            "--route", "POST /data", "cat",
        ]);
        assert_eq!(args.routes, vec![
            "GET /hello", "echo hello",
            "POST /data", "cat",
        ]);
    }

    #[test]
    fn test_no_routes() {
        let args = Args::parse_from(["sherut"]);
        assert!(args.routes.is_empty());
    }

    #[test]
    fn test_combined_options() {
        let args = Args::parse_from([
            "sherut",
            "--port", "9000",
            "--log-level", "warn",
            "--shell", "fish",
            "--header-format", "json",
            "--route", "/api", "echo api",
        ]);
        assert_eq!(args.port, 9000);
        assert!(matches!(args.log_level, LogLevel::Warn));
        assert_eq!(args.shell, Some(ShellType::Fish));
        assert_eq!(args.header_format, Some(HeaderFormat::Json));
        assert_eq!(args.routes, vec!["/api", "echo api"]);
    }
}
