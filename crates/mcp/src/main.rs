use rmcp::{ServiceExt, transport::stdio};
use shrimply_mcp::server::ShrimplyServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args_os().nth(1).is_some() {
        eprintln!("usage: shrimply-mcp");
        std::process::exit(2);
    }

    let service = ShrimplyServer::new().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
