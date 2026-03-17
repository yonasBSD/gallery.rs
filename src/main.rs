// src/main.rs
use gallery_rs::{config::Config, run};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> miette::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("info"))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Parse configuration from CLI
    let config = Config::from_args();

    // Hand off execution to the library's run function
    run(config).await
}
