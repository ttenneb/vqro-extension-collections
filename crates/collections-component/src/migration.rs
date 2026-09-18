//! Pure legacy-to-policy migration planning. No host calls or writes occur here.

use crate::policy::{decode_policy_value, PolicyRecord, NAMESPACE};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const MAX_SOURCE_BYTES: usize = 128 * 1024;
pub const LEGACY_CONTRACT: &str = "vqro.collections.legacy.v1";
pub const MIGRATION_CONTRACT: &str = "vqro.collections.migration.v1";
pub const MARKER_KEY: &str = "migration:legacy-v1";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyState {
    contract: String,
    source_generation: u64,
    collections: BTreeMap<String, LegacyCollection>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyCollection {
    #[serde(default, deserialize_with = "optional_non_null_label")]
    label: Option<String>,
    archived_terminal_ids: Vec<String>,
}

fn optional_non_null_label<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(deserializer).map(Some)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MigrationPlanV1 {
    pub contract: &'static str,
    pub namespace: &'static str,
    pub source_digest_sha256: String,
    pub source_generation: u64,
    pub values: BTreeMap<String, Value>,
    pub marker: MigrationMarkerV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MigrationMarkerV1 {
    pub key: &'static str,
    pub value: MigrationMarkerValueV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MigrationMarkerValueV1 {
    pub schema: &'static str,
    pub source_digest_sha256: String,
    pub source_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MigrationError {
    Invalid,
}

/// Adapts one exact legacy snapshot into complete package policy values. Repeating
/// this function with the same bytes returns byte-identical values and marker.
pub fn migrate_legacy(source: &[u8]) -> Result<MigrationPlanV1, MigrationError> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(MigrationError::Invalid);
    }
    let legacy: LegacyState =
        serde_json::from_slice(source).map_err(|_| MigrationError::Invalid)?;
    if legacy.contract != LEGACY_CONTRACT
        || legacy.source_generation == 0
        || legacy.collections.len() > 512
    {
        return Err(MigrationError::Invalid);
    }
    // The digest binds the exact persisted source bytes, not a repaired/reformatted value.
    let source_digest_sha256 = format!("{:x}", Sha256::digest(source));
    let mut values = BTreeMap::new();
    for (container_id, old) in legacy.collections {
        let record = PolicyRecord {
            schema: "vqro.collections.policy".into(),
            schema_version: 1,
            container_id: container_id.clone(),
            label: old.label,
            archived_terminal_ids: old.archived_terminal_ids,
        };
        let value = serde_json::to_value(record).map_err(|_| MigrationError::Invalid)?;
        decode_policy_value(&container_id, &value).map_err(|_| MigrationError::Invalid)?;
        values.insert(container_id, value);
    }
    let marker_value = MigrationMarkerValueV1 {
        schema: MIGRATION_CONTRACT,
        source_digest_sha256: source_digest_sha256.clone(),
        source_generation: legacy.source_generation,
    };
    Ok(MigrationPlanV1 {
        contract: MIGRATION_CONTRACT,
        namespace: NAMESPACE,
        source_digest_sha256,
        source_generation: legacy.source_generation,
        values,
        marker: MigrationMarkerV1 {
            key: MARKER_KEY,
            value: marker_value,
        },
    })
}
