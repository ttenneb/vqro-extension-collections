//! Native-testable service dispatcher with one injectable read-only host bridge.

use crate::projection::{
    project, valid_producer, valid_scope, HostDocument, ProjectionError, RenderParams,
    TerminalSnapshot, MAX_DOCUMENT_BYTES, MAX_SNAPSHOT_BYTES,
};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};

pub const SERVICE_ID: &str = "collections";
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
    let params = validate_request(&request)?;

    if bridge.cancelled() {
        return Err(InvokeError::Cancelled);
    }

    let call_id = call_id(request_bytes);
    let host_call = RuntimeHostCall {
        r#type: "host_call",
        call_id: &call_id,
        identity: &request.identity,
        depth: 1,
        capability: "host.terminals.read",
        method: "terminals.snapshot",
        namespace: NAMESPACE,
        params: SnapshotParams {
            contract: "host.terminals.v1",
        },
    };
    let host_call_bytes =
        serde_json::to_vec(&host_call).map_err(|_| InvokeError::InvalidRequest)?;
    if host_call_bytes.len() > MAX_FRAME_BYTES {
        return Err(InvokeError::InvalidRequest);
    }

    // Store the result so cancellation is checked after every attempted call,
    // including a bridge-level error, before any response is inspected.
    let response_result = bridge.call(&host_call_bytes);
    if bridge.cancelled() {
        return Err(InvokeError::Cancelled);
    }
    let response_bytes = response_result.map_err(|_| InvokeError::HostCallFailed)?;
    let snapshot = decode_response(&response_bytes, &call_id, &request.identity)?;
    let document = project(params, &snapshot).map_err(map_projection_error)?;
    encode_document(&document)
}

fn validate_request(request: &RuntimeRequest) -> Result<RenderParams, InvokeError> {
    let identity = &request.identity;
    if request.params.get().len() > 2_048 {
        return Err(InvokeError::InvalidRequest);
    }
    let params: RenderParams =
        serde_json::from_str(request.params.get()).map_err(|_| InvokeError::InvalidRequest)?;
    if request.contract != SERVICE_CONTRACT
        || request.method != METHOD
        || identity.package_id != PACKAGE_ID
        || identity.namespace != NAMESPACE
        || identity.service_id != SERVICE_ID
        || identity.generation == 0
        || request.request_id.is_empty()
        || request.request_id.len() > MAX_REQUEST_ID_BYTES
        || !opaque_id(&request.request_id)
        || params.contract != "host.document.render.v2"
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

fn decode_response(
    bytes: &[u8],
    expected_call_id: &str,
    expected_identity: &RuntimeIdentity,
) -> Result<TerminalSnapshot, InvokeError> {
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(InvokeError::InvalidHostResponse);
    }
    let response: RuntimeHostCallResponse =
        serde_json::from_slice(bytes).map_err(|_| InvokeError::InvalidHostResponse)?;
    if response.r#type != "host_call_response"
        || response.call_id != expected_call_id
        || response.identity != *expected_identity
    {
        return Err(InvokeError::InvalidHostResponse);
    }
    match (response.result, response.error) {
        (Some(result), None) => {
            if result.get().len() > MAX_SNAPSHOT_BYTES {
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

fn encode_document(document: &HostDocument) -> Result<Vec<u8>, InvokeError> {
    let bytes = serde_json::to_vec(document).map_err(|_| InvokeError::InvalidDocument)?;
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(InvokeError::InvalidDocument);
    }
    Ok(bytes)
}

fn call_id(request: &[u8]) -> String {
    format!("collections-{:x}", Sha256::digest(request))
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
