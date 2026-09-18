//! Native-testable dispatcher with bounded profile reads and one-read typed action planning.

use crate::action::{self, ActionError};
use crate::policy::{decode_namespace, NAMESPACE as POLICY_NAMESPACE};
#[cfg(test)]
use crate::projection::{project, HostDocument, MAX_DOCUMENT_BYTES};
use crate::projection::{
    valid_producer, valid_scope, ProjectionError, RenderParams, StateSnapshot, TerminalSnapshot,
    MAX_SNAPSHOT_BYTES, MAX_STATE_SNAPSHOT_BYTES,
};
use crate::r0;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};

pub const SERVICE_ID: &str = "collections";
#[cfg(test)]
pub const METHOD: &str = "host.document.render";
pub const PACKAGE_ID: &str = "vqro.collections";
pub const NAMESPACE: &str = "vqro.collections";
const SERVICE_CONTRACT: &str = "vqro.service.v1";
const MAX_FRAME_BYTES: usize = 262_144;
const MAX_REQUEST_ID_BYTES: usize = 128;

pub trait HostBridge {
    type Error;

    fn cancelled(&mut self) -> bool;
    fn call(&mut self, request: &[u8]) -> Result<Vec<u8>, Self::Error>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvokeError {
    InvalidRequest,
    Cancelled,
    HostCallFailed,
    InvalidHostResponse,
    InvalidSnapshot,
    InvalidDocument,
    Stale,
    Revoked,
}

impl InvokeError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::Cancelled => "cancelled",
            Self::HostCallFailed => "host_call_failed",
            Self::InvalidHostResponse => "invalid_host_response",
            Self::InvalidSnapshot => "invalid_snapshot",
            Self::InvalidDocument => "invalid_document",
            Self::Stale => "stale",
            Self::Revoked => "revoked",
        }
    }

    pub const fn message(self) -> &'static str {
        match self {
            Self::InvalidRequest => "request rejected",
            Self::Cancelled => "request cancelled",
            Self::HostCallFailed => "host call failed",
            Self::InvalidHostResponse => "host response rejected",
            Self::InvalidSnapshot => "terminal snapshot rejected",
            Self::InvalidDocument => "document rejected",
            Self::Stale => "observation is stale",
            Self::Revoked => "authority revoked",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeIdentity {
    pub package_id: String,
    pub namespace: String,
    pub service_id: String,
    pub generation: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeRequest {
    contract: String,
    request_id: String,
    identity: RuntimeIdentity,
    method: String,
    params: Box<RawValue>,
}

#[derive(Serialize)]
struct SnapshotParams {
    contract: &'static str,
}

#[derive(Serialize)]
struct RuntimeHostCall<'a> {
    r#type: &'static str,
    call_id: &'a str,
    identity: &'a RuntimeIdentity,
    depth: u8,
    capability: &'static str,
    method: &'static str,
    namespace: &'static str,
    params: SnapshotParams,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeErrorBody {
    code: String,
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeHostCallResponse {
    r#type: String,
    call_id: String,
    identity: RuntimeIdentity,
    #[serde(default)]
    result: Option<Box<RawValue>>,
    #[serde(default)]
    error: Option<RuntimeErrorBody>,
}

pub fn invoke<B: HostBridge>(bridge: &mut B, request_bytes: &[u8]) -> Result<Vec<u8>, InvokeError> {
    if request_bytes.len() > MAX_FRAME_BYTES {
        return Err(InvokeError::InvalidRequest);
    }
    let request: RuntimeRequest =
        serde_json::from_slice(request_bytes).map_err(|_| InvokeError::InvalidRequest)?;
    validate_envelope(&request)?;
    let params = if request.method == action::METHOD {
        None
    } else {
        Some(validate_render_request(&request)?)
    };

    if bridge.cancelled() {
        return Err(InvokeError::Cancelled);
    }

    let request_digest = format!("{:x}", Sha256::digest(request_bytes));
    let state_call_id = format!("collections-state-{request_digest}");
    let state_bytes = perform_call(
        bridge,
        &request.identity,
        &state_call_id,
        "host.state.read",
        "state.snapshot",
        "host.state.v1",
    )?;
    let state: StateSnapshot = decode_result(
        &state_bytes,
        &state_call_id,
        &request.identity,
        MAX_STATE_SNAPSHOT_BYTES,
    )?;
    if state.contract != "host.state.v1"
        || state.namespace != POLICY_NAMESPACE
        || !valid_state_identifier(&state.store_id, false)
        || !valid_state_identifier(&state.namespace, true)
        || state.store_generation == 0
        || state.revision == u64::MAX
        || serde_json::to_value(&state).map_err(|_| InvokeError::InvalidSnapshot)?
            != serde_json::from_slice::<serde_json::Value>(&state_bytes)
                .ok()
                .and_then(|value| value.get("result").cloned())
                .ok_or(InvokeError::InvalidSnapshot)?
    {
        return Err(InvokeError::InvalidSnapshot);
    }
    let policies = decode_namespace(&state.values).map_err(|_| InvokeError::InvalidSnapshot)?;
    if request.method == action::METHOD {
        let plan =
            action::plan(request.params.get().as_bytes(), &state).map_err(|error| match error {
                ActionError::Invalid => InvokeError::InvalidRequest,
                ActionError::Stale => InvokeError::Stale,
            })?;
        return encode_bounded(&plan);
    }
    if bridge.cancelled() {
        return Err(InvokeError::Cancelled);
    }

    let terminal_call_id = format!("collections-terminals-{request_digest}");
    let terminal_bytes = perform_call(
        bridge,
        &request.identity,
        &terminal_call_id,
        "host.terminals.read",
        "terminals.snapshot",
        "host.terminals.v1",
    )?;
    let snapshot: TerminalSnapshot = decode_result(
        &terminal_bytes,
        &terminal_call_id,
        &request.identity,
        MAX_SNAPSHOT_BYTES,
    )?;
    let params = params.expect("non-action methods have validated render params");
    #[cfg(test)]
    if request.method == METHOD {
        let document =
            project(params, &state, &policies, &snapshot).map_err(map_projection_error)?;
        return encode_document(&document);
    }
    let profile =
        r0::project_profile(params, &state, &policies, &snapshot).map_err(map_projection_error)?;
    if request.method == r0::PROFILE_METHOD {
        encode_bounded(&profile)
    } else {
        encode_bounded(&r0::declare_actions(&profile))
    }
}

fn validate_envelope(request: &RuntimeRequest) -> Result<(), InvokeError> {
    let identity = &request.identity;
    if request.contract != SERVICE_CONTRACT
        || !supported_method(&request.method)
        || identity.package_id != PACKAGE_ID
        || identity.namespace != NAMESPACE
        || identity.service_id != SERVICE_ID
        || identity.generation == 0
        || request.request_id.is_empty()
        || request.request_id.len() > MAX_REQUEST_ID_BYTES
        || !opaque_id(&request.request_id)
    {
        return Err(InvokeError::InvalidRequest);
    }
    Ok(())
}

fn supported_method(method: &str) -> bool {
    let supported = matches!(
        method,
        r0::PROFILE_METHOD | r0::ACTIONS_METHOD | action::METHOD
    );
    #[cfg(test)]
    let supported = supported || method == METHOD;
    supported
}

fn render_method(method: &str) -> bool {
    let supported = matches!(method, r0::PROFILE_METHOD | r0::ACTIONS_METHOD);
    #[cfg(test)]
    let supported = supported || method == METHOD;
    supported
}

fn validate_render_request(request: &RuntimeRequest) -> Result<RenderParams, InvokeError> {
    let identity = &request.identity;
    if request.params.get().len() > 2_048 || !render_method(&request.method) {
        return Err(InvokeError::InvalidRequest);
    }
    let params: RenderParams =
        serde_json::from_str(request.params.get()).map_err(|_| InvokeError::InvalidRequest)?;
    if params.contract != "host.document.render.v2"
        || params.producer.package_id != PACKAGE_ID
        || params.producer.service_id != SERVICE_ID
        || params.producer.runtime_generation != identity.generation
        || !valid_producer(&params.producer)
        || !valid_scope(&params.scope)
        || params.requested_revision == 0
        || serde_json::to_vec(&params).map_or(true, |encoded| encoded.len() > 2_048)
    {
        return Err(InvokeError::InvalidRequest);
    }
    Ok(params)
}

fn perform_call<B: HostBridge>(
    bridge: &mut B,
    identity: &RuntimeIdentity,
    call_id: &str,
    capability: &'static str,
    method: &'static str,
    contract: &'static str,
) -> Result<Vec<u8>, InvokeError> {
    let call = RuntimeHostCall {
        r#type: "host_call",
        call_id,
        identity,
        depth: 1,
        capability,
        method,
        namespace: NAMESPACE,
        params: SnapshotParams { contract },
    };
    let bytes = canonical_json_bytes(&call).map_err(|_| InvokeError::InvalidRequest)?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(InvokeError::InvalidRequest);
    }
    let response = bridge.call(&bytes);
    if bridge.cancelled() {
        return Err(InvokeError::Cancelled);
    }
    response.map_err(|_| InvokeError::HostCallFailed)
}

fn decode_result<T: for<'de> Deserialize<'de>>(
    bytes: &[u8],
    expected_call_id: &str,
    expected_identity: &RuntimeIdentity,
    max_result_bytes: usize,
) -> Result<T, InvokeError> {
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(InvokeError::InvalidHostResponse);
    }
    let response: RuntimeHostCallResponse =
        serde_json::from_slice(bytes).map_err(|_| InvokeError::InvalidHostResponse)?;
    if response.r#type != "host_response"
        || response.call_id != expected_call_id
        || response.identity != *expected_identity
    {
        return Err(InvokeError::InvalidHostResponse);
    }
    match (response.result, response.error) {
        (Some(result), None) => {
            if result.get().len() > max_result_bytes {
                return Err(InvokeError::InvalidHostResponse);
            }
            serde_json::from_str(result.get()).map_err(|_| InvokeError::InvalidSnapshot)
        }
        (None, Some(error)) => {
            // Read fields to ensure the strict error object is fully decoded,
            // but deliberately do not expose host-controlled strings.
            let _ = (error.code.len(), error.message.len());
            Err(InvokeError::HostCallFailed)
        }
        _ => Err(InvokeError::InvalidHostResponse),
    }
}

fn valid_state_identifier(value: &str, require_dot: bool) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && (!require_dot || value.contains('.'))
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':'))
}

fn encode_bounded(value: &impl Serialize) -> Result<Vec<u8>, InvokeError> {
    let bytes = canonical_json_bytes(value).map_err(|_| InvokeError::InvalidDocument)?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(InvokeError::InvalidDocument);
    }
    Ok(bytes)
}

#[cfg(test)]
fn encode_document(document: &HostDocument) -> Result<Vec<u8>, InvokeError> {
    let bytes = canonical_json_bytes(document).map_err(|_| InvokeError::InvalidDocument)?;
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(InvokeError::InvalidDocument);
    }
    Ok(bytes)
}

// This must remain byte-for-byte equivalent to the host component boundary's
// `extension_runtime::component::canonical_json_bytes` implementation.
fn canonical_json_bytes(value: &impl Serialize) -> Result<Vec<u8>, serde_json::Error> {
    fn sort(value: serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(object) => {
                let mut entries = object.into_iter().collect::<Vec<_>>();
                entries.sort_by(|left, right| left.0.cmp(&right.0));
                serde_json::Value::Object(
                    entries
                        .into_iter()
                        .map(|(key, value)| (key, sort(value)))
                        .collect(),
                )
            }
            serde_json::Value::Array(values) => {
                serde_json::Value::Array(values.into_iter().map(sort).collect())
            }
            value => value,
        }
    }

    serde_json::to_vec(&sort(serde_json::to_value(value)?))
}

fn opaque_id(value: &str) -> bool {
    value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-".contains(&byte))
}

fn map_projection_error(error: ProjectionError) -> InvokeError {
    match error {
        ProjectionError::SnapshotInvalid | ProjectionError::FingerprintInvalid => {
            InvokeError::InvalidSnapshot
        }
        ProjectionError::DocumentInvalid => InvokeError::InvalidDocument,
    }
}
