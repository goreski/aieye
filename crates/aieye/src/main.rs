//! The `aieye` command.

mod device;

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use clap::{Parser, Subcommand};
use tracing::Level;

#[derive(Parser)]
#[command(version, about = "Shows what AI tools can reach on this device")]
struct Cli {
    /// Folder for AIEye's own data [default: AIEye in the local app data folder]
    #[arg(long, global = true, value_name = "DIR")]
    data_dir: Option<PathBuf>,

    /// Log details to stderr
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Detect AI tools on this device and print the findings as JSON
    Scan {
        /// Folder with AI tool definitions
        #[arg(long, value_name = "DIR", default_value = "definitions")]
        definitions: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_writer(io::stderr)
        .with_max_level(if cli.verbose {
            Level::DEBUG
        } else {
            Level::WARN
        })
        .init();

    let data_dir = match cli.data_dir {
        Some(dir) => dir,
        None => dirs::data_local_dir()
            .context("cannot find the local app data folder; pass --data-dir")?
            .join("AIEye"),
    };

    match cli.command {
        Command::Scan { definitions } => scan(&data_dir, &definitions),
    }
}

fn scan(data_dir: &Path, definitions: &Path) -> Result<()> {
    let definitions = aieye_defs::load_dir(definitions).context("cannot load definitions")?;
    let device_id = device::load_or_create(data_dir)?;
    let ctx = aieye_collect::Context::current(device_id, definitions)?;
    let snapshot = aieye_collect::scan(&ctx)?;

    let mut out = io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, &snapshot)?;
    writeln!(out)?;
    Ok(())
}
