//! AIEye's data model.
//!
//! Every fact AIEye records is a [`Record`]: what was observed ([`Fact`]), on which
//! device, from which [`Source`], when, and how far it can be trusted ([`Evidence`]).

use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// How far a fact can be trusted (concept §5.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Evidence {
    /// Recorded by the OS or by the service being accessed.
    Verified,
    /// A tool's own configuration or a vendor's own list.
    Declared,
    /// Produced by AIEye's deterministic detection rules.
    Inferred,
}

/// Identifies one device. Created at random on first run, never derived from hardware.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeviceId(Uuid);

impl DeviceId {
    pub fn random() -> Self {
        Self(Uuid::new_v4())
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl FromStr for DeviceId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self)
    }
}

/// Identifies an organisation. Set only when the device joins an org console.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OrgId(pub String);

/// Where a fact came from, e.g. `os:filesystem` or `os:process-list`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Source(String);

impl Source {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What was observed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Fact {
    /// A file or folder that a definition links to an AI tool exists.
    ToolPresent { tool: String, path: PathBuf },
    /// A process is running from an executable that a definition links to an AI tool.
    ToolRunning {
        tool: String,
        pid: u32,
        exe: PathBuf,
    },
}

/// One fact, with the fields every record carries (concept §12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub device_id: DeviceId,
    pub org_id: Option<OrgId>,
    pub source: Source,
    pub evidence: Evidence,
    pub observed_at: Timestamp,
    pub fact: Fact,
}

/// The records from one scan of a device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub device_id: DeviceId,
    pub taken_at: Timestamp,
    pub records: Vec<Record>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> Record {
        Record {
            device_id: DeviceId::random(),
            org_id: None,
            source: Source::new("os:process-list"),
            evidence: Evidence::Verified,
            observed_at: "2026-10-26T12:00:00Z".parse().unwrap(),
            fact: Fact::ToolRunning {
                tool: "codex-cli".into(),
                pid: 42,
                exe: PathBuf::from("codex.exe"),
            },
        }
    }

    #[test]
    fn evidence_labels_serialize_in_lowercase() {
        let labels = [Evidence::Verified, Evidence::Declared, Evidence::Inferred]
            .map(|evidence| serde_json::to_string(&evidence).unwrap());
        assert_eq!(labels, ["\"verified\"", "\"declared\"", "\"inferred\""]);
    }

    #[test]
    fn records_carry_the_required_fields() {
        let json = serde_json::to_value(record()).unwrap();
        for field in ["device_id", "org_id", "source", "evidence", "observed_at"] {
            assert!(json.get(field).is_some(), "missing {field}");
        }
        assert!(json["org_id"].is_null());
        assert_eq!(json["source"], "os:process-list");
        assert_eq!(json["observed_at"], "2026-10-26T12:00:00Z");
        assert_eq!(json["fact"]["kind"], "tool_running");
    }

    #[test]
    fn records_round_trip_through_json() {
        let record = record();
        let json = serde_json::to_string(&record).unwrap();
        assert_eq!(serde_json::from_str::<Record>(&json).unwrap(), record);
    }

    #[test]
    fn device_ids_parse_their_display_form() {
        let id = DeviceId::random();
        assert_eq!(id.to_string().parse::<DeviceId>().unwrap(), id);
    }
}
