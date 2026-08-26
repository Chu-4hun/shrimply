use std::path::PathBuf;

use rmcp::{ServiceExt, transport::stdio};
use shrimply_mcp::{bridge::Bridge, server::ShrimplyServer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let Some(project_path) = args.next().map(PathBuf::from) else {
        eprintln!("usage: shrimply-mcp PROJECT.shrimp");
        std::process::exit(2);
    };
    if args.next().is_some() {
        eprintln!("usage: shrimply-mcp PROJECT.shrimp");
        std::process::exit(2);
    }

    let bridge = Bridge::connect(&project_path).map_err(|error| {
        eprintln!("shrimply-mcp: {error}");
        error
    })?;
    let service = ShrimplyServer::new(bridge).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
