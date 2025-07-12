use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;

/// Try to extract a <call_mcp name="tool">...</call_mcp> block from the text.
/// Returns `(tool_name, args_json)` on success.
pub fn extract_mcp_call(text: &str) -> Option<Result<(String, Value)>> {
    static RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r#"<call_mcp\s+name="([^"]+)">\s*([\s\S]*?)\s*</call_mcp>"#).unwrap()
    });

    let caps = RE.captures(text)?;
    let name = caps.get(1)?.as_str().to_string();
    let json_str = caps.get(2)?.as_str();
    match serde_json::from_str::<Value>(json_str) {
        Ok(v) => Some(Ok((name, v)) ),
        Err(e) => Some(Err(anyhow!("invalid json in <call_mcp>: {}", e))),
    }
}
