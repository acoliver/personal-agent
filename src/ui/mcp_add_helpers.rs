//! Shared helpers for MCP add view.

use std::fs::OpenOptions;
use std::io::Write;

pub fn log_to_file(message: &str) {
    let log_path = dirs::home_dir()
        .unwrap_or_default()
        .join("Library/Application Support/PersonalAgent/debug.log");

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&log_path) {
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let _ = writeln!(file, "[{timestamp}] McpAddView: {message}");
    }
}

/// Parsed MCP URL result
#[derive(Debug, Clone)]
pub enum ParsedMcp {
    Npm {
        identifier: String,
        runtime_hint: String,
    },
    Docker {
        image: String,
    },
    Http {
        url: String,
    },
}

/// Parse MCP URL to detect package type
pub fn parse_mcp_url(url: &str) -> Result<ParsedMcp, String> {
    let url = url.trim();

    // npx -y @package/name or npx @package/name
    if url.starts_with("npx ") {
        let parts: Vec<&str> = url.split_whitespace().collect();
        // Find the package identifier - it's not "npx" and not a flag (starts with -)
        let identifier = parts
            .iter()
            .skip(1) // Skip "npx"
            .find(|p| !p.starts_with('-'))
            .ok_or("Invalid npx command")?;
        return Ok(ParsedMcp::Npm {
            identifier: identifier.to_string(),
            runtime_hint: "npx".to_string(),
        });
    }

    // docker run image
    if url.starts_with("docker ") {
        let parts: Vec<&str> = url.split_whitespace().collect();
        let image = parts.last().ok_or("Invalid docker command")?;
        return Ok(ParsedMcp::Docker {
            image: image.to_string(),
        });
    }

    // HTTP URL
    if url.starts_with("http://") || url.starts_with("https://") {
        return Ok(ParsedMcp::Http {
            url: url.to_string(),
        });
    }

    // Bare package name (assume npm)
    if url.starts_with('@') || url.contains('/') {
        return Ok(ParsedMcp::Npm {
            identifier: url.to_string(),
            runtime_hint: "npx".to_string(),
        });
    }

    Err("Unrecognized URL format".to_string())
}
