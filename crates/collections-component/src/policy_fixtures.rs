//! Test-only parity model for prospective Collections label/archive policy.
//!
//! This module and its fixture are package-owned and provisional. They do not describe a public
//! host state API and are not linked to component runtime behavior.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

const POLICY_SCHEMA: &str = "vqro.collections.policy";
const POLICY_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    fixture: String,
    status: String,
    authority: Vec<String>,
    input: PolicyInput,
    expected_canonical_policy: CanonicalPolicy,
    externally_supplied_neutral_order: Vec<String>,
    expected_projection: Projection,
    expected_stale_archive_retention: Vec<String>,
    invalid_container_ids: Vec<String>,
    invalid_terminal_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyInput {
    container_id: String,
    label: Option<String>,
    archived_terminal_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CanonicalPolicy {
    schema: String,
    schema_version: u32,
    container_id: String,
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

fn fixture() -> Fixture {
    serde_json::from_slice(include_bytes!(
        "../fixtures/provisional-policy-parity-v1.json"
    ))
    .expect("valid provisional policy fixture")
}

fn canonicalize(input: &PolicyInput) -> Result<CanonicalPolicy, &'static str> {
    if !valid_container_id(&input.container_id)
        || input
            .archived_terminal_ids
            .iter()
            .any(|terminal_id| !valid_terminal_id(terminal_id))
    {
        return Err("invalid identity");
    }

    let mut archived_terminal_ids = input.archived_terminal_ids.clone();
    archived_terminal_ids.sort();
    if archived_terminal_ids
        .windows(2)
        .any(|pair| pair[0] == pair[1])
    {
        return Err("duplicate archive identity");
    }

    Ok(CanonicalPolicy {
        schema: POLICY_SCHEMA.into(),
        schema_version: POLICY_SCHEMA_VERSION,
        container_id: input.container_id.clone(),
        label: input.label.clone(),
        archived_terminal_ids,
    })
}

fn reconcile(
    neutral_order: &[String],
    policy: &CanonicalPolicy,
) -> Result<Projection, &'static str> {
    let mut neutral_ids = BTreeSet::new();
    if neutral_order
        .iter()
        .any(|terminal_id| !valid_terminal_id(terminal_id) || !neutral_ids.insert(terminal_id))
    {
        return Err("invalid neutral order");
    }
    let archived = policy
        .archived_terminal_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();

    Ok(Projection {
        label: policy.label.clone(),
        terminals: neutral_order
            .iter()
            .map(|terminal_id| TerminalPolicy {
                archived: archived.contains(terminal_id.as_str()),
                terminal_id: terminal_id.clone(),
            })
            .collect(),
    })
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

fn assert_no_authority_fields(value: &Value) {
    const FORBIDDEN: &[&str] = &[
        "pane_id",
        "pane_ids",
        "members",
        "member_ids",
        "selected",
        "selected_terminal_id",
        "placement",
        "index",
        "focused",
        "workspace_id",
        "tab_id",
    ];
    match value {
        Value::Array(values) => values.iter().for_each(assert_no_authority_fields),
        Value::Object(fields) => {
            for (key, value) in fields {
                assert!(!FORBIDDEN.contains(&key.as_str()), "forbidden field {key}");
                assert_no_authority_fields(value);
            }
        }
        _ => {}
    }
}

#[test]
fn provisional_fixture_is_explicitly_package_owned_and_has_only_label_archive_authority() {
    let fixture_value: Value = serde_json::from_slice(include_bytes!(
        "../fixtures/provisional-policy-parity-v1.json"
    ))
    .unwrap();
    let fixture = fixture();

    assert_eq!(
        fixture.fixture,
        "vqro.collections.provisional-policy-parity.v1"
    );
    assert_eq!(
        fixture.status,
        "package-owned-test-only-not-a-host-contract"
    );
    assert_eq!(fixture.authority, ["label", "archive"]);
    assert_no_authority_fields(&fixture_value);
}

#[test]
fn policy_uses_exact_public_identities_and_canonical_archive_order() {
    let fixture = fixture();
    let before = fixture.input.archived_terminal_ids.clone();
    let canonical = canonicalize(&fixture.input).unwrap();

    assert_eq!(canonical, fixture.expected_canonical_policy);
    assert_eq!(fixture.input.archived_terminal_ids, before);
    assert!(valid_container_id(&canonical.container_id));
    assert!(canonical
        .archived_terminal_ids
        .iter()
        .all(|terminal_id| valid_terminal_id(terminal_id)));
    assert!(fixture
        .invalid_container_ids
        .iter()
        .all(|identity| !valid_container_id(identity)));
    assert!(fixture
        .invalid_terminal_ids
        .iter()
        .all(|identity| !valid_terminal_id(identity)));

    let mut permuted = fixture.input.clone();
    permuted.archived_terminal_ids.reverse();
    assert_eq!(canonicalize(&permuted).unwrap(), canonical);
}

#[test]
fn reconciliation_preserves_external_order_and_stale_archive_policy() {
    let fixture = fixture();
    let canonical = canonicalize(&fixture.input).unwrap();
    let before = canonical.clone();
    let projection = reconcile(&fixture.externally_supplied_neutral_order, &canonical).unwrap();

    assert_eq!(projection, fixture.expected_projection);
    assert_eq!(canonical, before, "reconciliation must not prune policy");
    assert_eq!(
        projection
            .terminals
            .iter()
            .map(|terminal| &terminal.terminal_id)
            .collect::<Vec<_>>(),
        fixture
            .externally_supplied_neutral_order
            .iter()
            .collect::<Vec<_>>()
    );
    assert_eq!(
        canonical
            .archived_terminal_ids
            .iter()
            .filter(|terminal_id| {
                !fixture
                    .externally_supplied_neutral_order
                    .contains(terminal_id)
            })
            .cloned()
            .collect::<Vec<_>>(),
        fixture.expected_stale_archive_retention
    );
}
