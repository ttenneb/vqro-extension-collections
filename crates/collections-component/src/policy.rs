//! Strict package-owned Collections label/archive policy decoder.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const NAMESPACE: &str = "vqro.collections";
const SCHEMA: &str = "vqro.collections.policy";
const MAX_LABEL_BYTES: usize = 4096;
const MAX_ARCHIVES: usize = 64;
const MAX_VALUES: usize = 512;
const MAX_VALUE_BYTES: usize = 32 * 1024;
const MAX_NAMESPACE_BYTES: usize = 128 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyRecord {
    pub schema: String,
    pub schema_version: u32,
    pub container_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub archived_terminal_ids: Vec<String>,
}

impl PolicyRecord {
    pub fn empty(container_id: &str) -> Self {
        Self {
            schema: SCHEMA.into(),
            schema_version: 1,
            container_id: container_id.into(),
            label: None,
            archived_terminal_ids: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyError {
    Invalid,
}

pub fn decode_namespace(
    values: &BTreeMap<String, Value>,
) -> Result<BTreeMap<String, PolicyRecord>, PolicyError> {
    if values.len() > MAX_VALUES
        || serde_json::to_vec(values)
            .map_err(|_| PolicyError::Invalid)?
            .len()
            > MAX_NAMESPACE_BYTES
    {
        return Err(PolicyError::Invalid);
    }
    let mut records = BTreeMap::new();
    for (key, value) in values {
        if !valid_container_id(key) {
            continue;
        }
        if serde_json::to_vec(value)
            .map_err(|_| PolicyError::Invalid)?
            .len()
            > MAX_VALUE_BYTES
        {
            return Err(PolicyError::Invalid);
        }
        records.insert(key.clone(), decode_policy_value(key, value)?);
    }
    Ok(records)
}

pub fn decode_policy_bytes(key: &str, bytes: &[u8]) -> Result<(PolicyRecord, Value), PolicyError> {
    // Decode the typed object first so duplicate known fields fail instead of
    // being collapsed by an intermediate generic JSON map.
    let direct: PolicyRecord = serde_json::from_slice(bytes).map_err(|_| PolicyError::Invalid)?;
    let value: Value = serde_json::from_slice(bytes).map_err(|_| PolicyError::Invalid)?;
    let validated = decode_policy_value(key, &value)?;
    if direct != validated {
        return Err(PolicyError::Invalid);
    }
    Ok((validated, value))
}

pub fn decode_policy_value(key: &str, value: &Value) -> Result<PolicyRecord, PolicyError> {
    if !valid_container_id(key)
        || serde_json::to_vec(value)
            .map_err(|_| PolicyError::Invalid)?
            .len()
            > MAX_VALUE_BYTES
    {
        return Err(PolicyError::Invalid);
    }
    let record: PolicyRecord =
        serde_json::from_value(value.clone()).map_err(|_| PolicyError::Invalid)?;
    if record.schema != SCHEMA
        || record.schema_version != 1
        || record.container_id != key
        || record
            .label
            .as_ref()
            .is_some_and(|label| label.len() > MAX_LABEL_BYTES)
        || record.archived_terminal_ids.len() > MAX_ARCHIVES
        || record
            .archived_terminal_ids
            .iter()
            .any(|id| !valid_terminal_id(id))
        || record
            .archived_terminal_ids
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || serde_json::to_value(&record).map_err(|_| PolicyError::Invalid)? != *value
    {
        return Err(PolicyError::Invalid);
    }
    Ok(record)
}

pub fn archived(record: Option<&PolicyRecord>, terminal_id: &str) -> bool {
    record.is_some_and(|record| {
        record
            .archived_terminal_ids
            .binary_search_by(|id| id.as_str().cmp(terminal_id))
            .is_ok()
    })
}

pub fn validate_neutral(ids: &[String]) -> Result<(), PolicyError> {
    if ids.len() > 64
        || ids.iter().any(|id| !valid_terminal_id(id))
        || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
    {
        return Err(PolicyError::Invalid);
    }
    Ok(())
}

fn valid_container_id(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("container_") else {
        return false;
    };
    value.len() == 26
        && lower_hex(hex, 16)
        && u64::from_str_radix(hex, 16).is_ok_and(|raw| raw != 0 && raw != u64::MAX)
}
fn valid_terminal_id(value: &str) -> bool {
    value
        .strip_prefix("term_")
        .is_some_and(|hex| value.len() == 37 && lower_hex(hex, 32))
}
fn lower_hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;

    #[derive(Deserialize)]
    struct ValidVector {
        input: ValidInput,
        expected_canonical_value: Value,
    }

    #[derive(Deserialize)]
    struct ValidInput {
        key: String,
    }

    #[derive(Deserialize)]
    struct InvalidVectors {
        cases: Vec<InvalidVector>,
        invalid_non_policy_keys: Vec<String>,
    }

    #[derive(Deserialize)]
    struct InvalidVector {
        key: String,
        value: Value,
    }

    fn record_value(key: &str, label: Option<String>, archives: Vec<String>) -> Value {
        let mut value = json!({
            "schema": SCHEMA,
            "schema_version": 1,
            "container_id": key,
            "archived_terminal_ids": archives,
        });
        if let Some(label) = label {
            value["label"] = Value::String(label);
        }
        value
    }

    #[test]
    fn production_decoder_ignores_every_noncanonical_key() {
        let unrelated = BTreeMap::from([
            ("other.package".into(), json!({"anything": true})),
            ("container_bad".into(), json!({"broken": true})),
            ("container_0000000000000000".into(), json!({"broken": true})),
            ("container_ffffffffffffffff".into(), json!({"broken": true})),
        ]);
        assert_eq!(decode_namespace(&unrelated).unwrap(), BTreeMap::new());
    }

    #[test]
    fn production_decoder_runs_every_pinned_profile_vector() {
        let valid: ValidVector = serde_json::from_slice(include_bytes!(
            "../../../profiles/fixtures/collections-policy-v1.json"
        ))
        .unwrap();
        let values = BTreeMap::from([(valid.input.key.clone(), valid.expected_canonical_value)]);
        assert_eq!(decode_namespace(&values).unwrap().len(), 1);

        let invalid: InvalidVectors = serde_json::from_slice(include_bytes!(
            "../../../profiles/fixtures/collections-policy-v1-invalid.json"
        ))
        .unwrap();
        for vector in invalid.cases {
            let values = BTreeMap::from([(vector.key.clone(), vector.value)]);
            if valid_container_id(&vector.key) {
                assert_eq!(decode_namespace(&values), Err(PolicyError::Invalid));
            } else {
                assert_eq!(decode_namespace(&values).unwrap(), BTreeMap::new());
            }
        }
        for key in invalid.invalid_non_policy_keys {
            let values = BTreeMap::from([(key, json!({"malformed": true}))]);
            assert_eq!(decode_namespace(&values).unwrap(), BTreeMap::new());
        }
    }

    #[test]
    fn production_decoder_enforces_exact_bounds() {
        let key = "container_0000000000000001";
        let exact = BTreeMap::from([(
            key.into(),
            record_value(key, Some("x".repeat(MAX_LABEL_BYTES)), Vec::new()),
        )]);
        assert!(decode_namespace(&exact).is_ok());
        let over_label = BTreeMap::from([(
            key.into(),
            record_value(key, Some("é".repeat(MAX_LABEL_BYTES / 2 + 1)), Vec::new()),
        )]);
        assert_eq!(decode_namespace(&over_label), Err(PolicyError::Invalid));
        let exact_archives = (0..MAX_ARCHIVES)
            .map(|index| format!("term_{index:032x}"))
            .collect();
        let exact_archive_count =
            BTreeMap::from([(key.into(), record_value(key, None, exact_archives))]);
        assert!(decode_namespace(&exact_archive_count).is_ok());
        let archives = (0..=MAX_ARCHIVES)
            .map(|index| format!("term_{index:032x}"))
            .collect();
        let too_many_archives = BTreeMap::from([(key.into(), record_value(key, None, archives))]);
        assert_eq!(
            decode_namespace(&too_many_archives),
            Err(PolicyError::Invalid)
        );

        let exact_key_count = (0..MAX_VALUES)
            .map(|index| (format!("unrelated-{index}"), Value::Null))
            .collect();
        assert_eq!(decode_namespace(&exact_key_count).unwrap(), BTreeMap::new());
        let mut too_many_keys = BTreeMap::new();
        for index in 0..=MAX_VALUES {
            too_many_keys.insert(format!("unrelated-{index}"), Value::Null);
        }
        assert_eq!(decode_namespace(&too_many_keys), Err(PolicyError::Invalid));
        let empty_namespace_value: BTreeMap<String, Value> =
            BTreeMap::from([("unrelated".into(), Value::String(String::new()))]);
        let overhead = serde_json::to_vec(&empty_namespace_value).unwrap().len();
        let exact_namespace = BTreeMap::from([(
            "unrelated".into(),
            Value::String("x".repeat(MAX_NAMESPACE_BYTES - overhead)),
        )]);
        assert_eq!(
            serde_json::to_vec(&exact_namespace).unwrap().len(),
            MAX_NAMESPACE_BYTES
        );
        assert_eq!(decode_namespace(&exact_namespace).unwrap(), BTreeMap::new());
        let oversized_namespace = BTreeMap::from([(
            "unrelated".into(),
            Value::String("x".repeat(MAX_NAMESPACE_BYTES - overhead + 1)),
        )]);
        assert_eq!(
            decode_namespace(&oversized_namespace),
            Err(PolicyError::Invalid)
        );
        let oversized_value =
            BTreeMap::from([(key.into(), json!({"padding": "x".repeat(MAX_VALUE_BYTES)}))]);
        assert_eq!(
            decode_namespace(&oversized_value),
            Err(PolicyError::Invalid)
        );
    }

    #[test]
    fn production_decoder_accepts_empty_label_and_sorted_archive_ids() {
        let key = "container_0000000000000001";
        let values = BTreeMap::from([(
            key.into(),
            json!({
                "schema": "vqro.collections.policy",
                "schema_version": 1,
                "container_id": key,
                "label": "",
                "archived_terminal_ids": ["term_00000000000000000000000000000001"]
            }),
        )]);
        let decoded = decode_namespace(&values).unwrap();
        assert_eq!(decoded[key].label.as_deref(), Some(""));
    }
}
