//! Pure package-owned document-action planner. It emits data; it never executes effects.

use crate::policy::{decode_policy_bytes, decode_policy_value, PolicyRecord, NAMESPACE};
use crate::projection::{state_dependency_identity, DocumentDependency};
use serde::{Deserialize, Serialize};
use serde_json::{value::RawValue, Value};
use std::collections::BTreeSet;

pub const METHOD: &str = "host.document.action.invoke";
pub const CONTRACT: &str = "vqro.collections.document-action.v1";
pub const EFFECT_PLAN_CONTRACT: &str = "vqro.effect-plan.v1";
pub const LABEL_ACTION: &str = "vqro.collections.label.set";
pub const ARCHIVE_ACTION: &str = "vqro.collections.terminal.archive";
pub const UNARCHIVE_ACTION: &str = "vqro.collections.terminal.unarchive";
const MAX_PAYLOAD_BYTES: usize = 8 * 1024;
const MAX_PARAMS_BYTES: usize = 192 * 1024;
const MAX_DEPENDENCIES: usize = 64;
const MAX_EFFECT_PLAN_BYTES: usize = 128 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionParams {
    pub contract: String,
    pub package_id: String,
    pub service_id: String,
    pub document_id: String,
    pub action_id: String,
    pub payload: Box<RawValue>,
    pub observed_dependencies: Vec<DocumentDependency>,
    pub authority_generation: u64,
    pub state: ActionState,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionState {
    pub namespace: String,
    pub store_id: String,
    pub store_generation: u64,
    pub revision: u64,
    pub key: String,
    /// `null` means the key was absent. Objects are decoded from their raw bytes
    /// so duplicate fields cannot be collapsed before policy validation.
    pub value: Box<RawValue>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectPlanV1 {
    pub contract: &'static str,
    pub package_id: &'static str,
    pub service_id: &'static str,
    pub document_id: String,
    pub action_id: String,
    pub authority_generation: u64,
    pub preconditions: Preconditions,
    pub effects: Vec<StateCasEffect>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Preconditions {
    pub authority_generation: u64,
    pub observed_dependencies: Vec<DocumentDependency>,
    pub state_namespace: &'static str,
    pub state_store_id: String,
    pub state_store_generation: u64,
    pub state_revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StateCasEffect {
    pub kind: &'static str,
    pub namespace: &'static str,
    pub key: String,
    pub expected_store_generation: u64,
    pub expected_revision: u64,
    pub expected_value: Value,
    pub value: Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionError {
    Invalid,
    Stale,
    Revoked,
}

pub fn plan(params_bytes: &[u8], runtime_generation: u64) -> Result<EffectPlanV1, ActionError> {
    if params_bytes.len() > MAX_PARAMS_BYTES {
        return Err(ActionError::Invalid);
    }
    let params: ActionParams =
        serde_json::from_slice(params_bytes).map_err(|_| ActionError::Invalid)?;
    validate_fence(&params, runtime_generation)?;

    let (current, expected_value) = if params.state.value.get() == "null" {
        (None, Value::Null)
    } else {
        let (record, value) =
            decode_policy_bytes(&params.state.key, params.state.value.get().as_bytes())
                .map_err(|_| ActionError::Invalid)?;
        (Some(record), value)
    };
    let updated = update(
        &params.action_id,
        params.payload.get().as_bytes(),
        &params.state.key,
        current,
    )?;
    let updated_value = serde_json::to_value(updated).map_err(|_| ActionError::Invalid)?;
    let effect = StateCasEffect {
        kind: "state.cas",
        namespace: NAMESPACE,
        key: params.state.key.clone(),
        expected_store_generation: params.state.store_generation,
        expected_revision: params.state.revision,
        expected_value,
        value: updated_value,
    };
    let plan = EffectPlanV1 {
        contract: EFFECT_PLAN_CONTRACT,
        package_id: "vqro.collections",
        service_id: "collections",
        document_id: params.document_id,
        action_id: params.action_id,
        authority_generation: params.authority_generation,
        preconditions: Preconditions {
            authority_generation: params.authority_generation,
            observed_dependencies: params.observed_dependencies,
            state_namespace: NAMESPACE,
            state_store_id: params.state.store_id,
            state_store_generation: params.state.store_generation,
            state_revision: params.state.revision,
        },
        effects: vec![effect],
    };
    if serde_json::to_vec(&plan).map_or(true, |bytes| bytes.len() > MAX_EFFECT_PLAN_BYTES) {
        return Err(ActionError::Invalid);
    }
    Ok(plan)
}

fn validate_fence(params: &ActionParams, runtime_generation: u64) -> Result<(), ActionError> {
    if params.authority_generation == 0 || params.authority_generation != runtime_generation {
        return Err(ActionError::Revoked);
    }
    if params.contract != CONTRACT
        || params.package_id != "vqro.collections"
        || params.service_id != "collections"
        || params.state.namespace != NAMESPACE
        || !valid_state_id(&params.state.store_id)
        || params.state.store_generation == 0
        || !valid_opaque_id(&params.document_id)
        || !valid_container_id(&params.state.key)
        || params.payload.get().len() > MAX_PAYLOAD_BYTES
        || params.observed_dependencies.is_empty()
        || params.observed_dependencies.len() > MAX_DEPENDENCIES
        || params
            .observed_dependencies
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || params.observed_dependencies.iter().any(|dependency| {
            !valid_contract(&dependency.contract)
                || !valid_opaque_id(&dependency.scope_id)
                || dependency.revision == 0
                || dependency.generation == 0
        })
    {
        return Err(ActionError::Invalid);
    }
    let expected_revision = params
        .state
        .revision
        .checked_add(1)
        .ok_or(ActionError::Invalid)?;
    let expected_dependency = state_dependency_identity(
        &params.state.store_id,
        &params.state.namespace,
        expected_revision,
        params.state.store_generation,
    );
    let state_dependencies = params
        .observed_dependencies
        .iter()
        .filter(|dependency| dependency.contract == "host.state.v1")
        .collect::<Vec<_>>();
    if state_dependencies.len() != 1 || *state_dependencies[0] != expected_dependency {
        return Err(ActionError::Stale);
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LabelPayload {
    container_id: String,
    label: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TerminalPayload {
    container_id: String,
    terminal_id: String,
}

fn update(
    action_id: &str,
    payload_bytes: &[u8],
    state_key: &str,
    current: Option<PolicyRecord>,
) -> Result<PolicyRecord, ActionError> {
    let mut record = current.unwrap_or_else(|| PolicyRecord::empty(state_key));
    match action_id {
        LABEL_ACTION => {
            let payload: LabelPayload =
                serde_json::from_slice(payload_bytes).map_err(|_| ActionError::Invalid)?;
            if payload.container_id != state_key {
                return Err(ActionError::Invalid);
            }
            record.label = match payload.label {
                Value::Null => None,
                Value::String(label) if label.len() <= 4096 => Some(label),
                _ => return Err(ActionError::Invalid),
            };
        }
        ARCHIVE_ACTION | UNARCHIVE_ACTION => {
            let payload: TerminalPayload =
                serde_json::from_slice(payload_bytes).map_err(|_| ActionError::Invalid)?;
            if payload.container_id != state_key || !valid_terminal_id(&payload.terminal_id) {
                return Err(ActionError::Invalid);
            }
            let mut ids = record
                .archived_terminal_ids
                .into_iter()
                .collect::<BTreeSet<_>>();
            if action_id == ARCHIVE_ACTION {
                ids.insert(payload.terminal_id);
            } else {
                ids.remove(&payload.terminal_id);
            }
            if ids.len() > 64 {
                return Err(ActionError::Invalid);
            }
            record.archived_terminal_ids = ids.into_iter().collect();
        }
        _ => return Err(ActionError::Invalid),
    }
    // Reuse the production decoder as the final canonical state invariant.
    let value = serde_json::to_value(&record).map_err(|_| ActionError::Invalid)?;
    decode_policy_value(state_key, &value).map_err(|_| ActionError::Invalid)
}

fn valid_container_id(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("container_") else {
        return false;
    };
    hex.len() == 16
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && u64::from_str_radix(hex, 16).is_ok_and(|raw| raw != 0 && raw != u64::MAX)
}

fn valid_terminal_id(value: &str) -> bool {
    value.strip_prefix("term_").is_some_and(|hex| {
        hex.len() == 32
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn valid_state_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':'))
}

fn valid_opaque_id(value: &str) -> bool {
    (1..=128).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-".contains(&byte))
}

fn valid_contract(value: &str) -> bool {
    value.len() <= 128
        && value.split('.').count() >= 2
        && value.split('.').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}
