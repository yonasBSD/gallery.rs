// src/main.rs
use gallery_rs::{config::Config, run};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> miette::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        // Check RUST_LOG first, and default to "info" if RUST_LOG is empty/unset
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    tracing::debug!("tracing initialized");

    // Parse configuration from CLI
    let config = Config::from_args();

    // Hand off execution to the library's run function
    run(config).await
}
