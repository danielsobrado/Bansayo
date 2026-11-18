use bansayo_engine::{config, run};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;
#[derive(Parser)]
#[command(version, about = "Bansayo local motion tracking engine")]
struct Args {
    #[arg(long, default_value = "config/sim-full-body.yaml")]
    config: PathBuf,
    /// Validate configuration and exit without starting tracking or the API.
    #[arg(long)]
    check: bool,
    /// Also shut down gracefully when stdin closes, for a supervising process:
    /// Windows cannot deliver Ctrl-C to a child.
    #[arg(long)]
    shutdown_on_stdin_close: bool,
    /// Print the system cameras with the indices a `camera` source's `device` takes, and exit.
    #[arg(long)]
    list_cameras: bool,
    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Re-analyse every golden session of a `bansayo.golden/v1` manifest and check expectations.
    Regress {
        manifest: PathBuf,
        /// Write the measured values into the manifest instead of failing on differences.
        #[arg(long)]
        bless: bool,
    },
    /// Copy a stored take into a real golden set (28 §2) and add it to its manifest.
    Promote {
        /// The engine's data directory (`engine.data_dir`).
        #[arg(long)]
        data: PathBuf,
        /// Session id, e.g. 20261203T180000Z-k9x2.
        #[arg(long)]
        session: String,
        /// Golden set directory; `manifest.yaml` is created or extended there.
        #[arg(long)]
        golden: PathBuf,
        /// Human label of the rig, e.g. one-webcam-30.
        #[arg(long, default_value = "one-webcam")]
        rig: String,
    },
    /// Print the session times of hand claps in a session directory's motion.mcap, to align
    /// phone video with the session clock.
    Claps { session_dir: PathBuf },
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = Args::parse();
    match &args.command {
        Some(Cmd::Regress { manifest, bless }) => {
            std::process::exit(bansayo_engine::regress::main(manifest, *bless));
        }
        Some(Cmd::Promote {
            data,
            session,
            golden,
            rig,
        }) => match bansayo_engine::validation::promote(data, session, golden, rig) {
            Ok(()) => {
                println!(
                    "Promoted {session} to {}; annotate it, then run regress --bless",
                    golden.display()
                );
                return Ok(());
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        },
        Some(Cmd::Claps { session_dir }) => match bansayo_engine::validation::claps(session_dir) {
            Ok(times) if times.is_empty() => {
                eprintln!("No claps found: both hands must meet above the shoulders, in view.");
                std::process::exit(1);
            }
            Ok(times) => {
                for t in times {
                    println!("{t}");
                }
                return Ok(());
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        },
        None => {}
    }
    if args.list_cameras {
        match bansayo_engine::sources::camera::capture::list_devices() {
            Ok(cameras) if cameras.is_empty() => println!("No cameras found"),
            Ok(cameras) => {
                for (index, name) in cameras {
                    println!("{index}	{name}");
                    match bansayo_engine::sources::camera::capture::device_formats(index) {
                        Ok(formats) => formats.iter().for_each(|f| println!("	{f}")),
                        Err(e) => println!("	formats unavailable: {e}"),
                    }
                }
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
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
    let stdin_closed = async {
        if !args.shutdown_on_stdin_close {
            return std::future::pending().await;
        }
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        // A plain thread: a blocked stdin read must not hold the runtime open after Ctrl-C.
        std::thread::spawn(move || {
            let _ = std::io::copy(&mut std::io::stdin(), &mut std::io::sink());
            drop(tx);
        });
        let _ = rx.await;
    };
    run(config, async {
        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                if let Err(e) = result {
                    tracing::error!(%e,"Could not listen for Ctrl-C");
                }
            }
            () = stdin_closed => {}
        }
    })
    .await
}
