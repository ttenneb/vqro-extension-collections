//! Independent test-only implementation of the package-owned Collections policy profile.
//!
//! Nothing in this module is linked to component runtime behavior. The profile is not a host
//! contract and grants no state, terminal, document, persistence, or mutation authority.

use crate::policy as production;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const STATUS: &str = "package-owned-source-profile-not-host-contract";
const NAMESPACE: &str = "vqro.collections";
const POLICY_SCHEMA: &str = "vqro.collections.policy";
const POLICY_SCHEMA_VERSION: u32 = 1;
const MAX_LABEL_BYTES: usize = 4 * 1024;
const MAX_ARCHIVED_TERMINALS: usize = 64;
const MAX_NEUTRAL_TERMINALS: usize = 64;
const MAX_VALUE_BYTES: usize = 32 * 1024;
const MAX_NAMESPACE_KEYS: usize = 512;
const MAX_NAMESPACE_BYTES: usize = 128 * 1024;
const FINGERPRINT_DOMAIN: &[u8] = b"vqro.collections.membership-archive.v1\0";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ValidFixture {
    fixture: String,
    status: String,
    namespace: String,
    policy_fields: Vec<String>,
    input: PolicyInput,
    expected_canonical_value: PolicyRecord,
    expected_canonical_compact_json: String,
    expected_canonical_sha256: String,
    externally_supplied_neutral_order: Vec<String>,
    expected_projection: Projection,
    expected_stale_archive_retention: Vec<String>,
    expected_membership_archive_sha256: String,
    expected_missing_record_projection: Projection,
    expected_missing_record_fingerprint_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InvalidFixture {
    fixture: String,
    status: String,
    cases: Vec<InvalidCase>,
    invalid_non_policy_keys: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InvalidCase {
    name: String,
    key: String,
    value: Value,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyInput {
    key: String,
    label: Option<String>,
    archived_terminal_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PolicyRecord {
    schema: String,
    schema_version: u32,
    container_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    archived_terminal_ids: Vec<String>,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Projection {
    label: Option<String>,
    terminals: Vec<TerminalPolicy>,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct TerminalPolicy {
    terminal_id: String,
    archived: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PolicyError {
    InvalidNamespace,
    InvalidKey,
    InvalidValue,
    InvalidSchema,
    KeyValueMismatch,
    LabelTooLarge,
    TooManyArchives,
    InvalidTerminal,
    NoncanonicalArchives,
    TooManyNeutralTerminals,
    InvalidNeutralTerminal,
    DuplicateNeutralTerminal,
    ValueTooLarge,
    TooManyNamespaceValues,
    NamespaceTooLarge,
}

fn valid_fixture() -> ValidFixture {
    serde_json::from_slice(include_bytes!(
        "../../../profiles/fixtures/collections-policy-v1.json"
    ))
    .expect("valid package policy fixture")
}

fn invalid_fixture() -> InvalidFixture {
    serde_json::from_slice(include_bytes!(
        "../../../profiles/fixtures/collections-policy-v1-invalid.json"
    ))
    .expect("valid malformed-policy fixture")
}

fn construct(input: &PolicyInput) -> Result<PolicyRecord, PolicyError> {
    if !valid_container_id(&input.key) {
        return Err(PolicyError::InvalidKey);
    }
    let mut archived_terminal_ids = input.archived_terminal_ids.clone();
    archived_terminal_ids.sort();
    let record = PolicyRecord {
        schema: POLICY_SCHEMA.into(),
        schema_version: POLICY_SCHEMA_VERSION,
        container_id: input.key.clone(),
        label: input.label.clone(),
        archived_terminal_ids,
    };
    validate_record(&record)?;
    Ok(record)
}

fn decode_record(key: &str, value: &Value) -> Result<PolicyRecord, PolicyError> {
    if !valid_container_id(key) {
        return Err(PolicyError::InvalidKey);
    }
    ensure_value_size(value)?;
    let record: PolicyRecord =
        serde_json::from_value(value.clone()).map_err(|_| PolicyError::InvalidValue)?;
    if record.schema != POLICY_SCHEMA || record.schema_version != POLICY_SCHEMA_VERSION {
        return Err(PolicyError::InvalidSchema);
    }
    if record.container_id != key {
        return Err(PolicyError::KeyValueMismatch);
    }
    validate_record(&record)?;
    if encode_value(&record)? != *value {
        return Err(PolicyError::InvalidValue);
    }
    Ok(record)
}

fn decode_namespace(
    namespace: &str,
    values: &BTreeMap<String, Value>,
) -> Result<BTreeMap<String, PolicyRecord>, PolicyError> {
    if namespace != NAMESPACE {
        return Err(PolicyError::InvalidNamespace);
    }
    if values.len() > MAX_NAMESPACE_KEYS {
        return Err(PolicyError::TooManyNamespaceValues);
    }
    if serde_json::to_vec(values)
        .map_err(|_| PolicyError::InvalidValue)?
        .len()
        > MAX_NAMESPACE_BYTES
    {
        return Err(PolicyError::NamespaceTooLarge);
    }
    values
        .iter()
        .filter(|(key, _)| valid_container_id(key))
        .map(|(key, value)| decode_record(key, value).map(|record| (key.clone(), record)))
        .collect()
}

fn validate_record(record: &PolicyRecord) -> Result<(), PolicyError> {
    if record.schema != POLICY_SCHEMA || record.schema_version != POLICY_SCHEMA_VERSION {
        return Err(PolicyError::InvalidSchema);
    }
    if !valid_container_id(&record.container_id) {
        return Err(PolicyError::InvalidKey);
    }
    if record
        .label
        .as_ref()
        .is_some_and(|label| label.len() > MAX_LABEL_BYTES)
    {
        return Err(PolicyError::LabelTooLarge);
    }
    if record.archived_terminal_ids.len() > MAX_ARCHIVED_TERMINALS {
        return Err(PolicyError::TooManyArchives);
    }
    if record
        .archived_terminal_ids
        .iter()
        .any(|terminal_id| !valid_terminal_id(terminal_id))
    {
        return Err(PolicyError::InvalidTerminal);
    }
    if record
        .archived_terminal_ids
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(PolicyError::NoncanonicalArchives);
    }
    ensure_value_size(&encode_value(record)?)?;
    Ok(())
}

fn encode_value(record: &PolicyRecord) -> Result<Value, PolicyError> {
    serde_json::to_value(record).map_err(|_| PolicyError::InvalidValue)
}

fn canonical_bytes(record: &PolicyRecord) -> Result<Vec<u8>, PolicyError> {
    serde_json::to_vec(&encode_value(record)?).map_err(|_| PolicyError::InvalidValue)
}

fn ensure_value_size(value: &Value) -> Result<usize, PolicyError> {
    let size = serde_json::to_vec(value)
        .map_err(|_| PolicyError::InvalidValue)?
        .len();
    if size > MAX_VALUE_BYTES {
        return Err(PolicyError::ValueTooLarge);
    }
    Ok(size)
}

fn reconcile(
    container_id: &str,
    neutral_order: &[String],
    record: Option<&PolicyRecord>,
) -> Result<Projection, PolicyError> {
    if !valid_container_id(container_id) {
        return Err(PolicyError::InvalidKey);
    }
    if neutral_order.len() > MAX_NEUTRAL_TERMINALS {
        return Err(PolicyError::TooManyNeutralTerminals);
    }
    let mut neutral_ids = BTreeSet::new();
    for terminal_id in neutral_order {
        if !valid_terminal_id(terminal_id) {
            return Err(PolicyError::InvalidNeutralTerminal);
        }
        if !neutral_ids.insert(terminal_id.as_str()) {
            return Err(PolicyError::DuplicateNeutralTerminal);
        }
    }
    if let Some(record) = record {
        validate_record(record)?;
        if record.container_id != container_id {
            return Err(PolicyError::KeyValueMismatch);
        }
    }
    let archived = record
        .map(|record| {
            record
                .archived_terminal_ids
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    Ok(Projection {
        label: record.and_then(|record| record.label.clone()),
        terminals: neutral_order
            .iter()
            .map(|terminal_id| TerminalPolicy {
                archived: archived.contains(terminal_id.as_str()),
                terminal_id: terminal_id.clone(),
            })
            .collect(),
    })
}

fn membership_archive_fingerprint(
    container_id: &str,
    neutral_order: &[String],
    record: Option<&PolicyRecord>,
) -> Result<String, PolicyError> {
    let projection = reconcile(container_id, neutral_order, record)?;
    let raw = u64::from_str_radix(&container_id["container_".len()..], 16)
        .map_err(|_| PolicyError::InvalidKey)?;
    let mut digest = Sha256::new();
    digest.update(FINGERPRINT_DOMAIN);
    digest.update(raw.to_be_bytes());
    digest.update((projection.terminals.len() as u64).to_be_bytes());
    for terminal in projection.terminals {
        let encoded = terminal.terminal_id.as_bytes();
        digest.update((encoded.len() as u64).to_be_bytes());
        digest.update(encoded);
        digest.update([u8::from(terminal.archived)]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn valid_container_id(value: &str) -> bool {
    let Some(encoded) = value.strip_prefix("container_") else {
        return false;
    };
    value.len() == 26
        && lower_hex(encoded, 16)
        && u64::from_str_radix(encoded, 16).is_ok_and(|raw| raw != 0 && raw != u64::MAX)
}

fn valid_terminal_id(value: &str) -> bool {
    value
        .strip_prefix("term_")
        .is_some_and(|encoded| value.len() == 37 && lower_hex(encoded, 32))
}

fn lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn assert_no_host_authority_fields(value: &Value) {
    const FORBIDDEN: &[&str] = &[
        "capability",
        "dependencies",
        "host_call",
        "host_state",
        "focused",
        "index",
        "member_ids",
        "members",
        "pane_id",
        "pane_ids",
        "placement",
        "selected",
        "selected_terminal_id",
        "state.snapshot",
        "state.transact",
        "tab_id",
        "workspace_id",
    ];
    match value {
        Value::Array(values) => values.iter().for_each(assert_no_host_authority_fields),
        Value::Object(fields) => {
            for (key, value) in fields {
                assert!(!FORBIDDEN.contains(&key.as_str()), "forbidden field {key}");
                assert_no_host_authority_fields(value);
            }
        }
        _ => {}
    }
}

#[test]
fn profile_assets_are_explicitly_package_owned_source_only_and_strict() {
    let valid_value: Value = serde_json::from_slice(include_bytes!(
        "../../../profiles/fixtures/collections-policy-v1.json"
    ))
    .unwrap();
    let invalid_value: Value = serde_json::from_slice(include_bytes!(
        "../../../profiles/fixtures/collections-policy-v1-invalid.json"
    ))
    .unwrap();
    let schema: Value = serde_json::from_slice(include_bytes!(
        "../../../profiles/collections-policy-v1.schema.json"
    ))
    .unwrap();
    let fixture = valid_fixture();
    let invalid = invalid_fixture();

    assert_eq!(
        fixture.fixture,
        "vqro.collections.package-policy-profile.v1"
    );
    assert_eq!(
        invalid.fixture,
        "vqro.collections.package-policy-profile.invalid.v1"
    );
    assert_eq!(fixture.status, STATUS);
    assert_eq!(invalid.status, STATUS);
    assert_eq!(fixture.namespace, NAMESPACE);
    assert_eq!(fixture.policy_fields, ["label", "archive"]);
    assert_eq!(schema["x-status"], STATUS);
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(
        schema["properties"]["archived_terminal_ids"]["maxItems"],
        64
    );
    assert_eq!(schema["properties"]["label"]["maxLength"], 4096);
    assert_eq!(
        schema["required"],
        serde_json::json!([
            "schema",
            "schema_version",
            "container_id",
            "archived_terminal_ids"
        ])
    );
    assert_no_host_authority_fields(&valid_value);
    assert_no_host_authority_fields(&schema);
    assert_eq!(invalid_value["status"], STATUS);
}

#[test]
fn constructor_encoding_and_pinned_sha_are_canonical() {
    let fixture = valid_fixture();
    let before = fixture.input.archived_terminal_ids.clone();
    let record = construct(&fixture.input).unwrap();
    let bytes = canonical_bytes(&record).unwrap();

    assert_eq!(record, fixture.expected_canonical_value);
    assert_eq!(fixture.input.archived_terminal_ids, before);
    assert_eq!(
        String::from_utf8(bytes.clone()).unwrap(),
        fixture.expected_canonical_compact_json
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        fixture.expected_canonical_sha256
    );
    assert_eq!(
        decode_record(&fixture.input.key, &encode_value(&record).unwrap()).unwrap(),
        record
    );
}

#[test]
fn optional_label_is_omitted_never_null_and_empty_is_meaningful() {
    let fixture = valid_fixture();
    let mut without_label = fixture.input.clone();
    without_label.label = None;
    let absent = construct(&without_label).unwrap();
    let absent_value = encode_value(&absent).unwrap();
    assert!(absent_value.get("label").is_none());
    assert_eq!(
        decode_record(&without_label.key, &absent_value).unwrap(),
        absent
    );

    let mut empty_label = fixture.input;
    empty_label.label = Some(String::new());
    let empty = construct(&empty_label).unwrap();
    assert_eq!(empty.label.as_deref(), Some(""));
    assert_eq!(encode_value(&empty).unwrap()["label"], "");
}

#[test]
fn strict_decoder_rejects_every_pinned_malformed_case() {
    let fixture = invalid_fixture();
    for required in ["short-terminal", "wrong-terminal-identity-family"] {
        assert!(
            fixture.cases.iter().any(|case| case.name == required),
            "missing restored malformed case {required}"
        );
    }
    for case in fixture.cases {
        assert!(
            decode_record(&case.key, &case.value).is_err(),
            "accepted malformed case {}",
            case.name
        );
    }
    assert!(fixture
        .invalid_non_policy_keys
        .iter()
        .all(|key| !valid_container_id(key)));
}

#[test]
fn independent_and_production_namespace_decoders_cross_check() {
    let fixture = valid_fixture();
    let value = encode_value(&fixture.expected_canonical_value).unwrap();
    let values = BTreeMap::from([
        (fixture.input.key.clone(), value),
        (
            "container_0000000000000000".into(),
            serde_json::json!({"broken": true}),
        ),
        ("unrelated".into(), Value::Null),
    ]);
    let independent = decode_namespace(NAMESPACE, &values).unwrap();
    let runtime = production::decode_namespace(&values).unwrap();
    assert_eq!(
        serde_json::to_value(&runtime[&fixture.input.key]).unwrap(),
        encode_value(&independent[&fixture.input.key]).unwrap()
    );

    for case in invalid_fixture().cases {
        let values = BTreeMap::from([(case.key.clone(), case.value)]);
        assert_eq!(
            decode_namespace(NAMESPACE, &values).is_err(),
            production::decode_namespace(&values).is_err(),
            "decoder disagreement for {}",
            case.name
        );
    }
}

#[test]
fn namespace_decoder_is_exact_bounded_and_ignores_unrelated_keys() {
    let fixture = valid_fixture();
    let value = encode_value(&fixture.expected_canonical_value).unwrap();
    let values = BTreeMap::from([
        (fixture.input.key.clone(), value.clone()),
        (
            "unrelated-setting".into(),
            serde_json::json!({"kept": true}),
        ),
    ]);
    let decoded = decode_namespace(NAMESPACE, &values).unwrap();
    assert_eq!(decoded.len(), 1);
    assert_eq!(
        decoded[&fixture.input.key],
        fixture.expected_canonical_value
    );
    assert_eq!(
        decode_namespace("other.namespace", &values),
        Err(PolicyError::InvalidNamespace)
    );

    let mut too_many = BTreeMap::new();
    for index in 0..=MAX_NAMESPACE_KEYS {
        too_many.insert(format!("unrelated-{index:04}"), Value::Null);
    }
    assert_eq!(
        decode_namespace(NAMESPACE, &too_many),
        Err(PolicyError::TooManyNamespaceValues)
    );
    let oversized = BTreeMap::from([(
        "unrelated".into(),
        Value::String("x".repeat(MAX_NAMESPACE_BYTES)),
    )]);
    assert_eq!(
        decode_namespace(NAMESPACE, &oversized),
        Err(PolicyError::NamespaceTooLarge)
    );

    let malformed = BTreeMap::from([(fixture.input.key, serde_json::json!({"broken": true}))]);
    assert!(decode_namespace(NAMESPACE, &malformed).is_err());
}

#[test]
fn exact_bounds_and_noncanonical_wire_values_fail_closed() {
    let fixture = valid_fixture();
    let mut exact_label = fixture.input.clone();
    exact_label.label = Some("x".repeat(MAX_LABEL_BYTES));
    assert!(construct(&exact_label).is_ok());
    exact_label.label = Some("é".repeat(MAX_LABEL_BYTES / 2 + 1));
    assert_eq!(construct(&exact_label), Err(PolicyError::LabelTooLarge));

    let mut too_many_archives = fixture.input.clone();
    too_many_archives.archived_terminal_ids = (0..=MAX_ARCHIVED_TERMINALS)
        .map(|index| format!("term_{index:032x}"))
        .collect();
    assert_eq!(
        construct(&too_many_archives),
        Err(PolicyError::TooManyArchives)
    );

    let oversized = serde_json::json!({"padding": "x".repeat(MAX_VALUE_BYTES)});
    assert_eq!(
        decode_record(&fixture.input.key, &oversized),
        Err(PolicyError::ValueTooLarge)
    );
}

#[test]
fn reconciliation_preserves_external_order_and_stale_policy() {
    let fixture = valid_fixture();
    let record = construct(&fixture.input).unwrap();
    let before = record.clone();
    let projection = reconcile(
        &fixture.input.key,
        &fixture.externally_supplied_neutral_order,
        Some(&record),
    )
    .unwrap();

    assert_eq!(projection, fixture.expected_projection);
    assert_eq!(
        record, before,
        "reconciliation must not prune or mutate policy"
    );
    assert_eq!(
        record
            .archived_terminal_ids
            .iter()
            .filter(|terminal_id| !fixture
                .externally_supplied_neutral_order
                .contains(terminal_id))
            .cloned()
            .collect::<Vec<_>>(),
        fixture.expected_stale_archive_retention
    );
    assert_eq!(
        reconcile(
            &fixture.input.key,
            &fixture.externally_supplied_neutral_order,
            None,
        )
        .unwrap(),
        fixture.expected_missing_record_projection
    );
}

#[test]
fn package_owned_membership_archive_fingerprint_vectors_are_pinned() {
    let fixture = valid_fixture();
    let record = construct(&fixture.input).unwrap();
    assert_eq!(
        membership_archive_fingerprint(
            &fixture.input.key,
            &fixture.externally_supplied_neutral_order,
            Some(&record),
        )
        .unwrap(),
        fixture.expected_membership_archive_sha256
    );
    assert_eq!(
        membership_archive_fingerprint(
            &fixture.input.key,
            &fixture.externally_supplied_neutral_order,
            None,
        )
        .unwrap(),
        fixture.expected_missing_record_fingerprint_sha256
    );
}

#[test]
fn neutral_projection_bounds_and_identity_checks_are_independent() {
    let fixture = valid_fixture();
    let duplicate = vec![
        fixture.externally_supplied_neutral_order[0].clone(),
        fixture.externally_supplied_neutral_order[0].clone(),
    ];
    assert_eq!(
        reconcile(&fixture.input.key, &duplicate, None),
        Err(PolicyError::DuplicateNeutralTerminal)
    );
    let excessive = (0..=MAX_NEUTRAL_TERMINALS)
        .map(|index| format!("term_{index:032x}"))
        .collect::<Vec<_>>();
    assert_eq!(
        reconcile(&fixture.input.key, &excessive, None),
        Err(PolicyError::TooManyNeutralTerminals)
    );
}
