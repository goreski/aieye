//! Finds files and folders that definitions name.

use std::collections::BTreeSet;

use aieye_model::{Evidence, Fact, Record};
use anyhow::Result;
use jiff::Timestamp;

use crate::{Collector, Context};

/// Reports every existing file or folder that matches a definition's `detect` entries.
///
/// Folders AIEye cannot list, such as `C:\Program Files\WindowsApps` for a standard
/// user, are skipped.
pub struct Filesystem;

impl Collector for Filesystem {
    fn source(&self) -> &'static str {
        "os:filesystem"
    }

    fn collect(&self, ctx: &Context) -> Result<Vec<Record>> {
        let observed_at = Timestamp::now();
        let mut seen = BTreeSet::new();
        let mut records = Vec::new();
        for definition in &ctx.definitions {
            let Some(detect) = definition.detect.get(&ctx.platform) else {
                continue;
            };
            let tool = definition.id.as_str();
            for entry in detect.executables.iter().chain(&detect.paths) {
                for path in ctx.existing(tool, entry) {
                    if seen.insert((tool, path.clone())) {
                        let fact = Fact::ToolPresent {
                            tool: tool.to_owned(),
                            path,
                        };
                        records.push(ctx.record(
                            self.source(),
                            Evidence::Verified,
                            observed_at,
                            fact,
                        ));
                    }
                }
            }
        }
        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;
    use crate::tests::context;

    #[test]
    fn reports_existing_paths_only() {
        let home = tempfile::tempdir().unwrap();
        let exe = home.path().join("bin").join("1.0").join("tool.exe");
        fs::create_dir_all(exe.parent().unwrap()).unwrap();
        fs::write(&exe, "dummy").unwrap();
        fs::create_dir(home.path().join(".tool")).unwrap();
        let ctx = context(
            home.path(),
            r#"executables = ["~/bin/*/tool.exe"]
paths = ["~/.tool", "~/.missing"]"#,
        );

        let records = Filesystem.collect(&ctx).unwrap();

        let found: Vec<(&str, PathBuf)> = records
            .iter()
            .map(|record| match &record.fact {
                Fact::ToolPresent { tool, path } => (tool.as_str(), path.clone()),
                other => panic!("unexpected fact {other:?}"),
            })
            .collect();
        assert_eq!(
            found,
            [("test-tool", exe), ("test-tool", home.path().join(".tool"))]
        );
        assert!(records.iter().all(|record| {
            record.evidence == Evidence::Verified && record.source.as_str() == "os:filesystem"
        }));
    }
}
