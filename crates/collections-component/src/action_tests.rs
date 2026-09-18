use crate::action::*;
use crate::migration::*;
use crate::policy::decode_policy_value;
use crate::projection::{project, DocumentDependency, LayoutItem};
use crate::service::{self, HostBridge};
use crate::tests::{params, snapshot, state_snapshot, TERM1, TERM2};
use serde_json::{json, Value};

const KEY: &str = "container_0000000000000001";

struct NoCalls;
impl HostBridge for NoCalls {
    type Error = ();
    fn cancelled(&mut self) -> bool {
        false
    }
    fn call(&mut self, _request: &[u8]) -> Result<Vec<u8>, Self::Error> {
        panic!("action planning must not call the host")
    }
}

fn dependencies(revision: u64) -> Vec<DocumentDependency> {
    vec![
        DocumentDependency {
            contract: "host.state.v1".into(),
            scope_id: "state_c44bb5f411b1c3102a74c7fbd1b189ab9f186b39995aeab32dafb8e0e1addc2d".into(),
            revision: revision + 1,
            generation: 7,
        },
        DocumentDependency {
            contract: "host.terminals.v1".into(),
            scope_id: "tab_00000000000000000000000000000001/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            revision: 1,
            generation: 5,
        },
    ]
}

fn action(action_id: &str, payload: Value, value: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "contract": CONTRACT,
        "package_id": "vqro.collections",
        "service_id": "collections",
        "document_id": "collections/current",
        "action_id": action_id,
        "payload": payload,
        "observed_dependencies": dependencies(4),
        "authority_generation": 9,
        "state": {"namespace":"vqro.collections","store_id":"session:test","store_generation":7,"revision":4,"key":KEY,"value":value}
    }))
    .unwrap()
}

#[test]
fn action_service_dispatch_is_zero_ambient_and_zero_host_call() {
    let params: Value = serde_json::from_slice(include_bytes!(
        "../../../contracts/fixtures/collections-actions/label-invocation.json"
    ))
    .unwrap();
    let request = json!({
        "contract":"vqro.service.v1", "request_id":"action-1",
        "identity":{"package_id":"vqro.collections","namespace":"vqro.collections","service_id":"collections","generation":9},
        "method":METHOD, "params":params
    });
    let output = service::invoke(&mut NoCalls, &serde_json::to_vec(&request).unwrap()).unwrap();
    let output: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(output["contract"], EFFECT_PLAN_CONTRACT);
    assert_eq!(output["effects"][0]["kind"], "state.cas");
}

#[test]
fn pinned_label_contract_fixture_matches_planner_exactly() {
    let invocation =
        include_bytes!("../../../contracts/fixtures/collections-actions/label-invocation.json");
    let expected: Value = serde_json::from_slice(include_bytes!(
        "../../../contracts/fixtures/collections-actions/label-effect-plan.json"
    ))
    .unwrap();
    assert_eq!(
        serde_json::to_value(plan(invocation, 9).unwrap()).unwrap(),
        expected
    );
}

#[test]
fn label_archive_and_unarchive_emit_one_whole_value_cas() {
    let label = plan(
        &action(
            LABEL_ACTION,
            json!({"container_id":KEY,"label":"Ops"}),
            Value::Null,
        ),
        9,
    )
    .unwrap();
    assert_eq!(label.effects.len(), 1);
    assert_eq!(label.effects[0].kind, "state.cas");
    assert_eq!(label.effects[0].expected_store_generation, 7);
    assert_eq!(label.effects[0].expected_value, Value::Null);
    assert_eq!(label.effects[0].value["label"], "Ops");
    assert_eq!(label.preconditions.state_store_id, "session:test");
    assert_eq!(label.preconditions.state_store_generation, 7);
    assert_eq!(label.preconditions.state_revision, 4);
    assert_eq!(label.preconditions.observed_dependencies, dependencies(4));

    let archived = plan(
        &action(
            ARCHIVE_ACTION,
            json!({"container_id":KEY,"terminal_id":TERM2}),
            label.effects[0].value.clone(),
        ),
        9,
    )
    .unwrap();
    assert_eq!(
        archived.effects[0].value["archived_terminal_ids"],
        json!([TERM2])
    );
    let unarchived = plan(
        &action(
            UNARCHIVE_ACTION,
            json!({"container_id":KEY,"terminal_id":TERM2}),
            archived.effects[0].value.clone(),
        ),
        9,
    )
    .unwrap();
    assert_eq!(
        unarchived.effects[0].value["archived_terminal_ids"],
        json!([])
    );
    decode_policy_value(KEY, &unarchived.effects[0].value).unwrap();
}

#[test]
fn planned_policy_projects_deterministically_without_terminal_authority() {
    let planned = plan(
        &action(
            ARCHIVE_ACTION,
            json!({"container_id":KEY,"terminal_id":TERM2}),
            Value::Null,
        ),
        9,
    )
    .unwrap();
    let policy = decode_policy_value(KEY, &planned.effects[0].value).unwrap();
    let terminal_snapshot = snapshot(vec![LayoutItem::Container {
        container_id: KEY.into(),
        terminal_ids: vec![TERM1.into(), TERM2.into()],
        selected_terminal_id: Some(TERM1.into()),
    }]);
    let policies = std::collections::BTreeMap::from([(KEY.into(), policy)]);
    let first = project(params(), &state_snapshot(), &policies, &terminal_snapshot).unwrap();
    let second = project(params(), &state_snapshot(), &policies, &terminal_snapshot).unwrap();
    assert_eq!(first, second);
    let encoded = serde_json::to_string(&first).unwrap();
    assert!(encoded.contains("Archived terminal"));
    assert!(!encoded.contains(ARCHIVE_ACTION));
}

#[test]
fn stale_revoked_and_malformed_actions_fail_closed() {
    let valid = action(
        LABEL_ACTION,
        json!({"container_id":KEY,"label":"Ops"}),
        Value::Null,
    );
    assert_eq!(plan(&valid, 10), Err(ActionError::Revoked));
    for (field, replacement) in [
        ("revision", json!(4)),
        ("generation", json!(8)),
        (
            "scope_id",
            json!("state_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        ),
    ] {
        let mut stale: Value = serde_json::from_slice(&valid).unwrap();
        stale["observed_dependencies"][0][field] = replacement;
        assert_eq!(
            plan(&serde_json::to_vec(&stale).unwrap(), 9),
            Err(ActionError::Stale),
            "{field}"
        );
    }
    for payload in [
        json!({"container_id":KEY}),
        json!({"container_id":KEY,"label":"ok","unknown":true}),
        json!({"container_id":"container_0000000000000002","label":"wrong key"}),
    ] {
        assert_eq!(
            plan(&action(LABEL_ACTION, payload, Value::Null), 9),
            Err(ActionError::Invalid)
        );
    }
}

#[test]
fn legacy_migration_is_exact_and_idempotent() {
    let source =
        include_bytes!("../../../contracts/fixtures/collections-migration/legacy-input.json");
    let first = migrate_legacy(source).unwrap();
    let second = migrate_legacy(source).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.source_generation, 7);
    assert_eq!(first.marker.key, MARKER_KEY);
    assert_eq!(
        first.marker.value.source_digest_sha256,
        first.source_digest_sha256
    );
    let expected: Value = serde_json::from_slice(include_bytes!(
        "../../../contracts/fixtures/collections-migration/migration-plan.json"
    ))
    .unwrap();
    assert_eq!(serde_json::to_value(&first).unwrap(), expected);
    assert_eq!(first.values[KEY]["label"], "Helpers");
    assert_eq!(first.values[KEY]["archived_terminal_ids"], json!([TERM2]));

    let mut changed = source.to_vec();
    changed.push(b' ');
    let changed = migrate_legacy(&changed).unwrap();
    assert_ne!(changed.source_digest_sha256, first.source_digest_sha256);
    assert_eq!(changed.values, first.values);
}

#[test]
fn migration_rejects_unknown_or_noncanonical_legacy_state() {
    for source in [
        json!({"contract":LEGACY_CONTRACT,"source_generation":0,"collections":{}}),
        json!({"contract":LEGACY_CONTRACT,"source_generation":1,"collections":{KEY:{"archived_terminal_ids":[TERM2,TERM1]}}}),
        json!({"contract":LEGACY_CONTRACT,"source_generation":1,"collections":{},"unknown":true}),
    ] {
        assert_eq!(
            migrate_legacy(&serde_json::to_vec(&source).unwrap()),
            Err(MigrationError::Invalid)
        );
    }
}
