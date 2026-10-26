//! The device ID: created at random on first run and kept in AIEye's data folder.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use aieye_model::DeviceId;
use anyhow::{Context, Result};

const FILE_NAME: &str = "device-id";

/// Reads the device ID from `data_dir`, creating it on first run.
///
/// An unreadable or corrupt ID is an error rather than a reason to make a new one,
/// because a new ID would split the device's history in two.
pub fn load_or_create(data_dir: &Path) -> Result<DeviceId> {
    let path = data_dir.join(FILE_NAME);
    match fs::read_to_string(&path) {
        Ok(text) => text
            .trim()
            .parse()
            .with_context(|| format!("{} does not hold a valid device ID", path.display())),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            fs::create_dir_all(data_dir)
                .with_context(|| format!("cannot create {}", data_dir.display()))?;
            let id = DeviceId::random();
            fs::write(&path, format!("{id}\n"))
                .with_context(|| format!("cannot write {}", path.display()))?;
            Ok(id)
        }
        Err(error) => Err(error).with_context(|| format!("cannot read {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_the_id_once_and_then_reuses_it() {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path().join("AIEye");

        let first = load_or_create(&data_dir).unwrap();
        let second = load_or_create(&data_dir).unwrap();

        assert_eq!(first, second);
    }

    #[test]
    fn refuses_a_corrupt_id() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(FILE_NAME), "not an id").unwrap();

        assert!(load_or_create(dir.path()).is_err());
    }
}
