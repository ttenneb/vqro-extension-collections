//! Typed R0 Collections action planner. It emits one package-state CAS and executes no effect.

use crate::policy::{decode_policy_value, PolicyRecord, NAMESPACE};
use crate::projection::StateSnapshot;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

pub const METHOD: &str = "vqro.collections.action.plan.v1";
pub const INVOCATION_CONTRACT: &str = "vqro.collections.action-invocation.v1";
pub const EFFECT_PLAN_CONTRACT: &str = "vqro.collections.effect-plan.v1";
const MAX_INVOCATION_BYTES: usize = 32 * 1024;
const MAX_EFFECT_PLAN_BYTES: usize = 32 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionInvocation {
    pub contract: String,
    pub ticket: String,
    pub idempotency_key: String,
    pub document_revision: u64,
    pub actions_revision: u64,
    pub action_id: String,
    pub expected_store_generation: u64,
    pub expected_namespace_revision: u64,
    pub dependency: ActionDependency,
    pub input: ActionInput,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActionDependency {
    Container {
        container_id: String,
    },
    TerminalMembership {
        container_id: String,
        terminal_id: String,
        terminal_snapshot_fingerprint_sha256: String,
        provider_generation: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActionInput {
    SetLabel {
        container_id: String,
        #[serde(default)]
        label: Option<String>,
    },
    SetArchived {
        container_id: String,
        terminal_id: String,
        archived: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectPlan {
    pub contract: &'static str,
    pub ticket: String,
    pub idempotency_key: String,
    pub expected_store_generation: u64,
    pub expected_namespace_revision: u64,
    pub state: StateCas,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StateCas {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionError {
    Invalid,
    Stale,
}

pub fn plan(invocation_bytes: &[u8], state: &StateSnapshot) -> Result<EffectPlan, ActionError> {
    if invocation_bytes.len() > MAX_INVOCATION_BYTES {
        return Err(ActionError::Invalid);
    }
    let invocation: ActionInvocation =
        serde_json::from_slice(invocation_bytes).map_err(|_| ActionError::Invalid)?;
    let container_id = validate(&invocation)?;
    if state.contract != "host.state.v1"
        || state.namespace != NAMESPACE
        || state.store_generation != invocation.expected_store_generation
        || state.revision != invocation.expected_namespace_revision
    {
        return Err(ActionError::Stale);
    }

    let current = match state.values.get(&container_id) {
        Some(value) => {
            Some(decode_policy_value(&container_id, value).map_err(|_| ActionError::Invalid)?)
        }
        None => None,
    };
    let updated = update(&invocation.input, &container_id, current)?;
    let value = if updated.label.is_none() && updated.archived_terminal_ids.is_empty() {
        None
    } else {
        Some(serde_json::to_value(updated).map_err(|_| ActionError::Invalid)?)
    };
    let plan = EffectPlan {
        contract: EFFECT_PLAN_CONTRACT,
        ticket: invocation.ticket,
        idempotency_key: invocation.idempotency_key,
        expected_store_generation: invocation.expected_store_generation,
        expected_namespace_revision: invocation.expected_namespace_revision,
        state: StateCas {
            key: container_id,
            value,
        },
    };
    if serde_json::to_vec(&plan).map_or(true, |bytes| bytes.len() > MAX_EFFECT_PLAN_BYTES) {
        return Err(ActionError::Invalid);
    }
    Ok(plan)
}

fn validate(invocation: &ActionInvocation) -> Result<String, ActionError> {
    if invocation.contract != INVOCATION_CONTRACT
        || !valid_opaque(&invocation.ticket)
        || !valid_opaque(&invocation.idempotency_key)
        || !valid_opaque(&invocation.action_id)
        || invocation.document_revision == 0
        || invocation.actions_revision == 0
        || invocation.expected_store_generation == 0
    {
        return Err(ActionError::Invalid);
    }
    match (&invocation.input, &invocation.dependency) {
        (
            ActionInput::SetLabel {
                container_id,
                label,
            },
            ActionDependency::Container {
                container_id: dependency_container,
            },
        ) if container_id == dependency_container
            && valid_container_id(container_id)
            && invocation.action_id == format!("set-label:{container_id}")
            && label.as_ref().is_none_or(|label| {
                label.len() <= 4096 && !label.chars().any(char::is_control)
            }) =>
        {
            Ok(container_id.clone())
        }
        (
            ActionInput::SetArchived {
                container_id,
                terminal_id,
                ..
            },
            ActionDependency::TerminalMembership {
                container_id: dependency_container,
                terminal_id: dependency_terminal,
                terminal_snapshot_fingerprint_sha256,
                provider_generation,
            },
        ) if container_id == dependency_container
            && terminal_id == dependency_terminal
            && valid_container_id(container_id)
            && valid_terminal_id(terminal_id)
            && invocation.action_id == format!("set-archived:{terminal_id}")
            && lower_hex(terminal_snapshot_fingerprint_sha256, 64)
            && *provider_generation > 0 =>
        {
            Ok(container_id.clone())
        }
        _ => Err(ActionError::Invalid),
    }
}

fn update(
    input: &ActionInput,
    container_id: &str,
    current: Option<PolicyRecord>,
) -> Result<PolicyRecord, ActionError> {
    let mut record = current.unwrap_or_else(|| PolicyRecord::empty(container_id));
    match input {
        ActionInput::SetLabel { label, .. } => record.label.clone_from(label),
        ActionInput::SetArchived {
            terminal_id,
            archived,
            ..
        } => {
            let mut ids = record
                .archived_terminal_ids
                .into_iter()
                .collect::<BTreeSet<_>>();
            if *archived {
                ids.insert(terminal_id.clone());
            } else {
                ids.remove(terminal_id);
            }
            if ids.len() > 64 {
                return Err(ActionError::Invalid);
            }
            record.archived_terminal_ids = ids.into_iter().collect();
        }
    }
    let value = serde_json::to_value(&record).map_err(|_| ActionError::Invalid)?;
    decode_policy_value(container_id, &value).map_err(|_| ActionError::Invalid)
}

fn valid_opaque(value: &str) -> bool {
    (1..=256).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-".contains(&byte))
}

fn valid_container_id(value: &str) -> bool {
    value.strip_prefix("container_").is_some_and(|hex| {
        lower_hex(hex, 16)
            && u64::from_str_radix(hex, 16).is_ok_and(|raw| raw != 0 && raw != u64::MAX)
    })
}

fn valid_terminal_id(value: &str) -> bool {
    value
        .strip_prefix("term_")
        .is_some_and(|hex| lower_hex(hex, 32))
}

fn lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
