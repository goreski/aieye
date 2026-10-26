//! What the window can ask for. Scanning and reading the database block, so each
//! command runs on a worker thread.

use std::fs;

use aieye_defs::Platform;
use aieye_store::{Store, device};
use anyhow::{Context as _, Result};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::Paths;
use crate::inventory::{self, ScanView};

/// The latest saved scan, or nothing before the first one.
#[tauri::command]
pub async fn latest(paths: State<'_, Paths>) -> Result<Option<ScanView>, String> {
    let paths = paths.inner().clone();
    blocking(move || latest_view(&paths)).await
}

/// Scans the device, saves the snapshot and returns it.
#[tauri::command]
pub async fn scan(paths: State<'_, Paths>) -> Result<ScanView, String> {
    let paths = paths.inner().clone();
    blocking(move || {
        scan_and_save(&paths)?;
        latest_view(&paths)?.context("the scan was saved but cannot be read back")
    })
    .await
}

/// Asks where to save a snapshot as JSON and writes it there. Returns the path, or
/// nothing if the user cancelled.
#[tauri::command]
pub async fn export(
    app: AppHandle,
    paths: State<'_, Paths>,
    id: i64,
) -> Result<Option<String>, String> {
    let paths = paths.inner().clone();
    blocking(move || {
        let snapshot = Store::open_in(&paths.data_dir)?.load(id)?;
        let Some(target) = app
            .dialog()
            .file()
            .set_file_name(format!("aieye-snapshot-{id}.json"))
            .add_filter("JSON", &["json"])
            .blocking_save_file()
        else {
            return Ok(None);
        };
        let path = target.into_path()?;
        fs::write(&path, serde_json::to_vec_pretty(&snapshot)?)
            .with_context(|| format!("cannot write {}", path.display()))?;
        Ok(Some(path.display().to_string()))
    })
    .await
}

fn scan_and_save(paths: &Paths) -> Result<()> {
    let definitions =
        aieye_defs::load_dir(&paths.definitions).context("cannot load definitions")?;
    let device_id = device::load_or_create(&paths.data_dir)?;
    let ctx = aieye_collect::Context::current(device_id, definitions)?;
    let snapshot = aieye_collect::scan(&ctx)?;
    Store::open_in(&paths.data_dir)?.save(&snapshot)?;
    Ok(())
}

fn latest_view(paths: &Paths) -> Result<Option<ScanView>> {
    let store = Store::open_in(&paths.data_dir)?;
    let snapshots = store.list()?;
    let Some(latest) = snapshots.last() else {
        return Ok(None);
    };
    let snapshot = store.load(latest.id)?;
    let previous = match snapshots.iter().rev().nth(1) {
        Some(info) => {
            let before = store.load(info.id)?;
            Some((
                info.clone(),
                aieye_model::diff(&before.records, &snapshot.records),
            ))
        }
        None => None,
    };
    let definitions =
        aieye_defs::load_dir(&paths.definitions).context("cannot load definitions")?;
    let platform =
        Platform::current().context("AIEye does not support this operating system yet")?;
    Ok(Some(inventory::view(
        latest.clone(),
        &snapshot,
        &definitions,
        platform,
        previous,
    )))
}

/// Runs blocking work off the window's thread and turns errors into messages.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T, String> {
    match tauri::async_runtime::spawn_blocking(work).await {
        Ok(result) => result.map_err(|error| format!("{error:#}")),
        Err(error) => Err(error.to_string()),
    }
}
