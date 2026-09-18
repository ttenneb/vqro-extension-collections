use crate::action::{self, ActionError, ActionInvocation};
use crate::projection::StateSnapshot;
use serde_json::{json, Value};
use std::collections::BTreeMap;

const CONTAINER: &str = "container_000000000000002a";
const TERM2: &str = "term_22222222222222222222222222222222";

fn state() -> StateSnapshot {
    StateSnapshot {
        contract: "host.state.v1".into(),
        store_id: "store_alpha".into(),
        store_generation: 2,
        namespace: "vqro.collections".into(),
        revision: 3,
        sequence: 4,
        values: BTreeMap::from([(
            CONTAINER.into(),
            json!({
                "schema": "vqro.collections.policy",
                "schema_version": 1,
                "container_id": CONTAINER,
                "label": "Helpers",
                "archived_terminal_ids": [TERM2]
            }),
        )]),
    }
}

#[test]
fn frozen_label_invocation_produces_the_frozen_one_state_cas() {
    let invocation = include_bytes!(
        "../../../contracts/fixtures/vqro-collections-v1/invocation-set-label-valid.json"
    );
    let plan = action::plan(invocation, &state()).unwrap();
    let actual = serde_json::to_value(plan).unwrap();
    let expected: Value = serde_json::from_slice(include_bytes!(
        "../../../contracts/fixtures/vqro-collections-v1/effect-plan-valid.json"
    ))
    .unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn archive_and_restore_are_one_key_plans() {
    let invocation = include_bytes!(
        "../../../contracts/fixtures/vqro-collections-v1/invocation-set-archived-valid.json"
    );
    let plan = action::plan(invocation, &state()).unwrap();
    let value = plan.state.value.unwrap();
    assert_eq!(
        value["archived_terminal_ids"],
        json!(["term_11111111111111111111111111111111", TERM2])
    );

    let mut restore: Value = serde_json::from_slice(invocation).unwrap();
    restore["input"]["terminal_id"] = Value::String(TERM2.into());
    restore["input"]["archived"] = Value::Bool(false);
    restore["dependency"]["terminal_id"] = Value::String(TERM2.into());
    restore["action_id"] = Value::String(format!("set-archived:{TERM2}"));
    let plan = action::plan(&serde_json::to_vec(&restore).unwrap(), &state()).unwrap();
    assert_eq!(
        plan.state.value.unwrap()["archived_terminal_ids"],
        json!([])
    );
}

#[test]
fn strict_dependency_and_state_fences_fail_closed() {
    let invalid = include_bytes!(
        "../../../contracts/fixtures/vqro-collections-v1/invocation-set-label-invalid-dependency.json"
    );
    assert_eq!(action::plan(invalid, &state()), Err(ActionError::Invalid));

    let valid = include_bytes!(
        "../../../contracts/fixtures/vqro-collections-v1/invocation-set-label-valid.json"
    );
    let mut stale = state();
    stale.revision += 1;
    assert_eq!(action::plan(valid, &stale), Err(ActionError::Stale));
}

#[test]
fn unknown_fields_controls_and_cross_subject_ids_are_rejected() {
    let bytes = include_bytes!(
        "../../../contracts/fixtures/vqro-collections-v1/invocation-set-label-valid.json"
    );
    let mut invocation: Value = serde_json::from_slice(bytes).unwrap();
    invocation["extra"] = json!(true);
    assert_eq!(
        action::plan(&serde_json::to_vec(&invocation).unwrap(), &state()),
        Err(ActionError::Invalid)
    );

    let mut invocation: Value = serde_json::from_slice(bytes).unwrap();
    invocation["input"]["label"] = Value::String("bad\u{1b}".into());
    assert_eq!(
        action::plan(&serde_json::to_vec(&invocation).unwrap(), &state()),
        Err(ActionError::Invalid)
    );

    let mut invocation: ActionInvocation = serde_json::from_slice(bytes).unwrap();
    invocation.action_id = "set-label:container_000000000000002b".into();
    assert_eq!(
        action::plan(
            &serde_json::to_vec(&json!({
                "contract": invocation.contract,
                "ticket": invocation.ticket,
                "idempotency_key": invocation.idempotency_key,
                "document_revision": invocation.document_revision,
                "actions_revision": invocation.actions_revision,
                "action_id": invocation.action_id,
                "expected_store_generation": invocation.expected_store_generation,
                "expected_namespace_revision": invocation.expected_namespace_revision,
                "dependency": {"kind": "container", "container_id": CONTAINER},
                "input": {"kind": "set_label", "container_id": CONTAINER, "label": "Workers"}
            }))
            .unwrap(),
            &state()
        ),
        Err(ActionError::Invalid)
    );
}
