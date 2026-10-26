//! The `aieye` command.

mod report;

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use aieye_model::Record;
use aieye_store::{SnapshotInfo, Store, device};
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
    /// Detect AI tools on this device, save the snapshot and print it as JSON
    Scan {
        /// Folder with AI tool definitions
        #[arg(long, value_name = "DIR", default_value = "definitions")]
        definitions: PathBuf,

        /// Print the snapshot without saving it
        #[arg(long)]
        no_save: bool,
    },
    /// Show what changed between two saved snapshots
    Diff {
        /// The older snapshot [default: the one before --to]
        #[arg(long, value_name = "ID")]
        from: Option<i64>,

        /// The newer snapshot [default: the latest]
        #[arg(long, value_name = "ID")]
        to: Option<i64>,

        /// Print the changes as JSON
        #[arg(long)]
        json: bool,
    },
    /// List saved snapshots and their alerts
    History,
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
        Command::Scan {
            definitions,
            no_save,
        } => scan(&data_dir, &definitions, !no_save),
        Command::Diff { from, to, json } => diff(&data_dir, from, to, json),
        Command::History => history(&data_dir),
    }
}

fn scan(data_dir: &Path, definitions: &Path, save: bool) -> Result<()> {
    let definitions = aieye_defs::load_dir(definitions).context("cannot load definitions")?;
    let device_id = device::load_or_create(data_dir)?;
    let ctx = aieye_collect::Context::current(device_id, definitions)?;
    let snapshot = aieye_collect::scan(&ctx)?;

    let mut summary = None;
    if save {
        let mut store = Store::open_in(data_dir)?;
        let previous = store.list()?.last().map(|info| info.id);
        let id = store.save(&snapshot)?;
        summary = Some(match previous {
            Some(previous) => {
                let changes = aieye_model::diff(&store.load(previous)?.records, &snapshot.records);
                let alerts = changes.alerts();
                let plural = if alerts == 1 { "" } else { "s" };
                format!(
                    "Saved snapshot {id}: {alerts} alert{plural} since snapshot {previous}. \
                     Run `aieye diff` for details."
                )
            }
            None => format!("Saved snapshot {id}, the first one."),
        });
    }

    let mut out = io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, &snapshot)?;
    writeln!(out)?;
    // The summary goes to stderr, so stdout stays pure JSON.
    if let Some(summary) = summary {
        eprintln!("{summary}");
    }
    Ok(())
}

fn diff(data_dir: &Path, from: Option<i64>, to: Option<i64>, json: bool) -> Result<()> {
    let store = Store::open_in(data_dir)?;
    let snapshots = store.list()?;
    let Some(to) = to.or_else(|| snapshots.last().map(|info| info.id)) else {
        println!("No snapshots yet. Run `aieye scan` first.");
        return Ok(());
    };
    let from = match from {
        Some(from) => from,
        None => match snapshots.iter().rev().find(|info| info.id < to) {
            Some(info) => info.id,
            None => {
                println!("Snapshot {to} is the first one, so there is nothing to compare yet.");
                return Ok(());
            }
        },
    };
    let (from_info, to_info) = (find(&snapshots, from)?, find(&snapshots, to)?);
    let changes = aieye_model::diff(&store.load(from)?.records, &store.load(to)?.records);

    if json {
        let report = serde_json::json!({ "from": from_info, "to": to_info, "diff": changes });
        let mut out = io::stdout().lock();
        serde_json::to_writer_pretty(&mut out, &report)?;
        writeln!(out)?;
    } else {
        print!("{}", report::diff(from_info, to_info, &changes));
    }
    Ok(())
}

fn history(data_dir: &Path) -> Result<()> {
    let store = Store::open_in(data_dir)?;
    let snapshots = store.list()?;
    if snapshots.is_empty() {
        println!("No snapshots yet. Run `aieye scan` first.");
        return Ok(());
    }
    let mut alerts = Vec::with_capacity(snapshots.len());
    let mut previous: Option<Vec<Record>> = None;
    for info in &snapshots {
        let records = store.load(info.id)?.records;
        alerts.push(
            previous
                .as_deref()
                .map(|before| aieye_model::diff(before, &records).alerts()),
        );
        previous = Some(records);
    }
    print!("{}", report::history(&snapshots, &alerts));
    Ok(())
}

fn find(snapshots: &[SnapshotInfo], id: i64) -> Result<&SnapshotInfo> {
    snapshots
        .iter()
        .find(|info| info.id == id)
        .with_context(|| format!("there is no snapshot {id}"))
}
