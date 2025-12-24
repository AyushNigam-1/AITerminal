use anyhow::Result;
use filesystem_mcp_rust::start_server;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    // Optional: Get allowed directories from args or env (semicolon-separated absolute paths)
    let args: Vec<String> = env::args().collect();
    let allowed_dirs = if args.len() > 1 {
        args[1].clone()
    } else {
        ".".to_string() // Default to current directory for safety
    };

    // Run the MCP server with allowed dirs
    start_server(&allowed_dirs).await?;
    Ok(())
}
