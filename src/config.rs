// src/config.rs
// RESTORING ARCHITECTURE
use clap::Parser;
use std::path::PathBuf;
use tracing::{debug, trace};

#[allow(dead_code)]
fn _log_config_module() {
    trace!("config module loaded");
    debug!("Config defaults set");
}

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Config {
    #[arg(short, long, default_value = "3020")]
    pub port: u16,

    #[arg(short = 'H', long, default_value = "127.0.0.1")]
    pub host: String,

    #[arg(short, long, default_value = "./photos")]
    pub storage_dir: PathBuf,

    #[arg(short, long)]
    pub verbose: bool,
}

impl Config {
    pub fn from_args() -> Self {
        Self::parse()
    }

    /// Helper for tests and logging to get the formatted address
    pub fn server_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 3020,
            host: "127.0.0.1".to_string(),
            storage_dir: PathBuf::from("./photos"),
            verbose: false,
        }
    }
}
