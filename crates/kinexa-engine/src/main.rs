use clap::Parser;
use kinexa_engine::{config, run};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;
#[derive(Parser)]
#[command(version, about = "Kinexa local motion tracking engine")]
struct Args {
    #[arg(long, default_value = "config/sim-full-body.yaml")]
    config: PathBuf,
    /// Validate configuration and exit without starting tracking or the API.
    #[arg(long)]
    check: bool,
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = Args::parse();
    let config = match config::load(&args.config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(config.config.logging.level.as_str()));
    match config.config.logging.format {
        config::LogFormat::Json => {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .json()
                .try_init()?;
        }
        config::LogFormat::Pretty => {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .compact()
                .try_init()?;
        }
    }
    if args.check {
        for warning in &config.warnings {
            eprintln!("Warning: {warning}");
        }
        println!("Configuration valid");
        return Ok(());
    }
    run(config, async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!(%e,"Could not listen for Ctrl-C");
        }
    })
    .await
}
