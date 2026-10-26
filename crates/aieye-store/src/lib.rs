//! AIEye's local store: saved snapshots in a SQLite file, and the device ID.

pub mod device;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use aieye_model::{DeviceId, Record, Snapshot};
use anyhow::{Context as _, Result, bail};
use jiff::Timestamp;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

/// The database file in AIEye's data folder.
pub const FILE_NAME: &str = "aieye.db";

/// AIEye's data folder for the signed-in user: `AIEye` in the local app data folder.
pub fn default_data_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|dir| dir.join("AIEye"))
}

/// Schema changes, applied in order. The database's `user_version` counts how many
/// have run. Never edit one that has shipped; add a new one.
const MIGRATIONS: &[&str] = &[
    // 1: snapshots and their records. Every record row carries the fields concept
    // §12 requires; `record` holds the whole record as JSON.
    "CREATE TABLE snapshots (
        id INTEGER PRIMARY KEY,
        device_id TEXT NOT NULL,
        taken_at TEXT NOT NULL,
        aieye_version TEXT NOT NULL
    );
    CREATE TABLE records (
        snapshot_id INTEGER NOT NULL REFERENCES snapshots (id) ON DELETE CASCADE,
        device_id TEXT NOT NULL,
        org_id TEXT,
        source TEXT NOT NULL,
        evidence TEXT NOT NULL,
        observed_at TEXT NOT NULL,
        kind TEXT NOT NULL,
        tool TEXT NOT NULL,
        fact_key TEXT NOT NULL,
        record TEXT NOT NULL
    );
    CREATE INDEX records_by_snapshot ON records (snapshot_id);
    CREATE INDEX records_by_key ON records (fact_key);",
];

/// A saved snapshot, without its records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SnapshotInfo {
    pub id: i64,
    pub device_id: DeviceId,
    pub taken_at: Timestamp,
    pub records: usize,
    pub aieye_version: String,
}

/// An open AIEye database.
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Opens the database in `data_dir`, creating the folder and file if needed.
    pub fn open_in(data_dir: &Path) -> Result<Self> {
        fs::create_dir_all(data_dir)
            .with_context(|| format!("cannot create {}", data_dir.display()))?;
        Self::open(&data_dir.join(FILE_NAME))
    }

    /// Opens the database at `path`, creating or upgrading its schema.
    pub fn open(path: &Path) -> Result<Self> {
        let mut conn =
            Connection::open(path).with_context(|| format!("cannot open {}", path.display()))?;
        conn.busy_timeout(Duration::from_secs(5))?;
        // Write-ahead logging lets the command line and the app use the file at once.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", true)?;
        migrate(&mut conn).with_context(|| format!("cannot upgrade {}", path.display()))?;
        Ok(Self { conn })
    }

    /// Saves a snapshot and returns its ID.
    pub fn save(&mut self, snapshot: &Snapshot) -> Result<i64> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO snapshots (device_id, taken_at, aieye_version) VALUES (?1, ?2, ?3)",
            params![
                snapshot.device_id.to_string(),
                snapshot.taken_at.to_string(),
                env!("CARGO_PKG_VERSION"),
            ],
        )?;
        let id = tx.last_insert_rowid();
        {
            let mut insert = tx.prepare(
                "INSERT INTO records (snapshot_id, device_id, org_id, source, evidence,
                     observed_at, kind, tool, fact_key, record)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )?;
            for record in &snapshot.records {
                insert.execute(params![
                    id,
                    record.device_id.to_string(),
                    record.org_id.as_ref().map(|org| org.0.as_str()),
                    record.source.as_str(),
                    label(&record.evidence)?,
                    record.observed_at.to_string(),
                    label(&record.fact)?,
                    record.fact.tool(),
                    serde_json::to_string(&record.fact.key())?,
                    serde_json::to_string(record)?,
                ])?;
            }
        }
        tx.commit()?;
        Ok(id)
    }

    /// All saved snapshots, oldest first.
    pub fn list(&self) -> Result<Vec<SnapshotInfo>> {
        let mut query = self.conn.prepare(
            "SELECT s.id, s.device_id, s.taken_at, s.aieye_version, COUNT(r.snapshot_id)
             FROM snapshots s LEFT JOIN records r ON r.snapshot_id = s.id
             GROUP BY s.id ORDER BY s.id",
        )?;
        let rows = query.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })?;
        let mut snapshots = Vec::new();
        for row in rows {
            let (id, device_id, taken_at, aieye_version, records) = row?;
            snapshots.push(SnapshotInfo {
                id,
                device_id: device_id.parse()?,
                taken_at: taken_at.parse()?,
                records: usize::try_from(records)?,
                aieye_version,
            });
        }
        Ok(snapshots)
    }

    /// One saved snapshot with its records.
    pub fn load(&self, id: i64) -> Result<Snapshot> {
        let header: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT device_id, taken_at FROM snapshots WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((device_id, taken_at)) = header else {
            bail!("there is no snapshot {id}");
        };
        let mut query = self
            .conn
            .prepare("SELECT record FROM records WHERE snapshot_id = ?1 ORDER BY rowid")?;
        let mut records = Vec::new();
        for json in query.query_map([id], |row| row.get::<_, String>(0))? {
            let record: Record = serde_json::from_str(&json?)
                .with_context(|| format!("snapshot {id} holds a record AIEye cannot read"))?;
            records.push(record);
        }
        Ok(Snapshot {
            device_id: device_id.parse()?,
            taken_at: taken_at.parse()?,
            records,
        })
    }
}

/// Brings the schema up to date. A database from a newer AIEye is refused rather
/// than guessed at.
fn migrate(conn: &mut Connection) -> Result<()> {
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let applied = usize::try_from(version)?;
    if applied > MIGRATIONS.len() {
        bail!(
            "the database has schema version {applied}, but this AIEye knows only up to {}",
            MIGRATIONS.len()
        );
    }
    let tx = conn.transaction()?;
    for migration in &MIGRATIONS[applied..] {
        tx.execute_batch(migration)?;
    }
    tx.pragma_update(None, "user_version", i64::try_from(MIGRATIONS.len())?)?;
    tx.commit()?;
    Ok(())
}

/// The `kind` tag or label a value serializes to, such as `verified` or `tool_present`.
fn label<T: Serialize>(value: &T) -> Result<String> {
    let value = serde_json::to_value(value)?;
    let label = value.as_str().or_else(|| value["kind"].as_str());
    Ok(label.context("value has no label")?.to_owned())
}

#[cfg(test)]
mod tests {
    use aieye_model::{Evidence, Fact, OrgId, Source};

    use super::*;

    fn snapshot(device_id: DeviceId, records: usize) -> Snapshot {
        let taken_at: Timestamp = "2026-10-26T12:00:00Z".parse().unwrap();
        Snapshot {
            device_id,
            taken_at,
            records: (0..records)
                .map(|n| Record {
                    device_id,
                    org_id: (n == 0).then(|| OrgId("org".into())),
                    source: Source::new("config"),
                    evidence: Evidence::Declared,
                    observed_at: taken_at,
                    fact: Fact::ConfiguredFolder {
                        tool: "codex-cli".into(),
                        file: "config.toml".into(),
                        path: format!("C:\\work\\{n}"),
                        trust: Some("trusted".into()),
                        boundary: false,
                    },
                })
                .collect(),
        }
    }

    #[test]
    fn saved_snapshots_load_back_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open_in(dir.path()).unwrap();
        let saved = snapshot(DeviceId::random(), 3);

        let id = store.save(&saved).unwrap();

        assert_eq!(store.load(id).unwrap(), saved);
    }

    #[test]
    fn lists_snapshots_oldest_first_with_record_counts() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open_in(dir.path()).unwrap();
        let device_id = DeviceId::random();
        let first = store.save(&snapshot(device_id, 2)).unwrap();
        let second = store.save(&snapshot(device_id, 0)).unwrap();

        let list = store.list().unwrap();

        let summary: Vec<_> = list.iter().map(|info| (info.id, info.records)).collect();
        assert_eq!(summary, [(first, 2), (second, 0)]);
        assert_eq!(list[0].device_id, device_id);
    }

    #[test]
    fn reopening_keeps_data_and_runs_no_migration_twice() {
        let dir = tempfile::tempdir().unwrap();
        let id = Store::open_in(dir.path())
            .unwrap()
            .save(&snapshot(DeviceId::random(), 1))
            .unwrap();

        let store = Store::open_in(dir.path()).unwrap();

        assert_eq!(store.load(id).unwrap().records.len(), 1);
    }

    #[test]
    fn refuses_a_database_from_a_newer_aieye() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);
        Connection::open(&path)
            .unwrap()
            .pragma_update(None, "user_version", 99)
            .unwrap();

        let error = Store::open(&path).err().expect("a newer schema is refused");

        assert!(format!("{error:#}").contains("schema version 99"));
    }

    #[test]
    fn loading_a_missing_snapshot_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open_in(dir.path()).unwrap();

        assert!(store.load(7).is_err());
    }
}
