use anyhow::Result;
use memds::Server;
use tokio::signal;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let server = Server::new();

    tracing::info!("Listening...");
    let server_service = server.service().await?;

    wait_for_signal().await;
    tracing::info!("SIGINT received, shutting down...");

    server_service.await;

    Ok(())
}

async fn wait_for_signal() {
    if let Err(e) = signal::ctrl_c().await {
        tracing::error!("Error waiting for SIGINT: {}", e);
    }
}
