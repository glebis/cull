use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(name = "cull")]
pub struct CliArgs {
    /// Start in tray-only mode (no window)
    #[arg(long)]
    pub tray: bool,

    /// Run as MCP stdio bridge
    #[arg(long)]
    pub mcp_stdio: bool,

    /// Enable MCP HTTP/SSE server on optional port (default: 9847)
    #[arg(long)]
    pub mcp_http: Option<Option<u16>>,

    /// HTTP listen host (default: 127.0.0.1)
    #[arg(long, default_value = "127.0.0.1")]
    pub mcp_http_host: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_default_args() {
        let args = CliArgs::try_parse_from(["cull"]).unwrap();
        assert!(!args.tray);
        assert!(!args.mcp_stdio);
        assert!(args.mcp_http.is_none());
        assert_eq!(args.mcp_http_host, "127.0.0.1");
    }

    #[test]
    fn test_tray_flag() {
        let args = CliArgs::try_parse_from(["cull", "--tray"]).unwrap();
        assert!(args.tray);
        assert!(!args.mcp_stdio);
    }

    #[test]
    fn test_mcp_stdio_flag() {
        let args = CliArgs::try_parse_from(["cull", "--mcp-stdio"]).unwrap();
        assert!(args.mcp_stdio);
        assert!(!args.tray);
    }

    #[test]
    fn test_mcp_http_no_port() {
        let args = CliArgs::try_parse_from(["cull", "--mcp-http"]).unwrap();
        assert!(args.mcp_http.is_some());
        assert_eq!(args.mcp_http.unwrap(), None);
    }

    #[test]
    fn test_mcp_http_with_port() {
        let args = CliArgs::try_parse_from(["cull", "--mcp-http", "8080"]).unwrap();
        assert_eq!(args.mcp_http, Some(Some(8080)));
    }

    #[test]
    fn test_mcp_http_host_custom() {
        let args = CliArgs::try_parse_from(["cull", "--mcp-http-host", "0.0.0.0"]).unwrap();
        assert_eq!(args.mcp_http_host, "0.0.0.0");
    }

    #[test]
    fn test_combined_flags() {
        let args = CliArgs::try_parse_from([
            "cull",
            "--tray",
            "--mcp-http",
            "9847",
            "--mcp-http-host",
            "0.0.0.0",
        ])
        .unwrap();
        assert!(args.tray);
        assert_eq!(args.mcp_http, Some(Some(9847)));
        assert_eq!(args.mcp_http_host, "0.0.0.0");
    }

    #[test]
    fn test_unknown_flag_errors() {
        let result = CliArgs::try_parse_from(["cull", "--bogus"]);
        assert!(result.is_err());
    }
}
