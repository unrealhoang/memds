
use anyhow::Result;
use memds::Server;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let server = Server::new();

    tracing::info!("Listening...");
    server.serve().await?;
    Ok(())
}
