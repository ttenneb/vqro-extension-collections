use crate::policy::PolicyRecord;
use crate::projection::*;
use crate::service::{self, HostBridge, InvokeError};
use serde_json::{json, Value};
use sha2::Digest;
use std::collections::VecDeque;

pub(crate) const TERM1: &str = "term_00000000000000000000000000000001";
pub(crate) const TERM2: &str = "term_00000000000000000000000000000002";
const TAB: &str = "tab_00000000000000000000000000000001";

fn fence() -> LeaseFence {
    LeaseFence {
        catalog_generation: 1,
        package_generation: 2,
        service_generation: 3,
        artifact_generation: 4,
        provider_generation: 5,
        runtime_generation: 6,
    }
}

pub(crate) fn snapshot(layout: Vec<LayoutItem>) -> TerminalSnapshot {
    let mut value = TerminalSnapshot {
        contract: "host.terminals.v1".into(),
        workspace_id: "w1".into(),
        tab_id: TAB.into(),
        lease_fence: fence(),
        fingerprint_sha256: "0".repeat(64),
        layout,
    };
    value.fingerprint_sha256 = canonical_fingerprint(&value).unwrap();
    value
}

pub(crate) fn state_snapshot() -> StateSnapshot {
    StateSnapshot {
        contract: "host.state.v1".into(),
        store_id: "session:test".into(),
        store_generation: 7,
        namespace: "vqro.collections".into(),
        revision: 0,
        sequence: 0,
        values: Default::default(),
    }
}

fn project_default(
    params: RenderParams,
    snapshot: &TerminalSnapshot,
) -> Result<HostDocument, ProjectionError> {
    project(params, &state_snapshot(), &Default::default(), snapshot)
}

pub(crate) fn params() -> RenderParams {
    RenderParams {
        contract: "host.document.render.v2".into(),
        producer: ProducerFence {
            package_id: "vqro.collections".into(),
            service_id: "collections".into(),
            artifact_sha256: "a".repeat(64),
            runtime_generation: 9,
            provider_generation: 10,
            scope_generation: 11,
        },
        scope: DocumentScope {
            contract: "host.collection.v1".into(),
            scope_id: "shadow/current".into(),
        },
        requested_revision: 12,
    }
}

fn request_value() -> Value {
    json!({
        "contract": "vqro.service.v1",
        "request_id": "request-1",
        "identity": {
            "package_id": "vqro.collections",
            "namespace": "vqro.collections",
            "service_id": "collections",
            "generation": 9
        },
        "method": "host.document.render",
        "params": params()
    })
}

#[derive(Clone)]
enum Reply {
    Snapshot(TerminalSnapshot),
    Envelope(Value),
    DuplicateSnapshot,
    Fail,
}

struct Bridge {
    reply: Reply,
    cancellation: VecDeque<bool>,
    calls: Vec<Value>,
    call_bytes: Vec<Vec<u8>>,
}

impl Bridge {
    fn new(reply: Reply) -> Self {
        Self {
            reply,
            cancellation: VecDeque::from([false, false, false, false]),
            calls: Vec::new(),
            call_bytes: Vec::new(),
        }
    }
}

#[derive(Clone)]
enum MethodReply {
    Valid,
    Result(Value),
    Error,
    WrongEnvelope,
    Envelope(Value),
    RawResult(usize),
}

struct MethodBridge {
    state: MethodReply,
    terminal: MethodReply,
    cancellation: VecDeque<bool>,
    calls: Vec<String>,
    attempts: std::collections::BTreeMap<String, usize>,
    transport_failure: Option<(String, usize)>,
}

impl MethodBridge {
    fn new(state: MethodReply, terminal: MethodReply) -> Self {
        Self {
            state,
            terminal,
            cancellation: VecDeque::from([false, false, false, false]),
            calls: Vec::new(),
            attempts: Default::default(),
            transport_failure: None,
        }
    }

    fn fail_transport(mut self, method: &str, attempt: usize) -> Self {
        self.transport_failure = Some((method.into(), attempt));
        self
    }
}

impl HostBridge for MethodBridge {
    type Error = ();

    fn cancelled(&mut self) -> bool {
        self.cancellation.pop_front().unwrap_or(false)
    }

    fn call(&mut self, request: &[u8]) -> Result<Vec<u8>, Self::Error> {
        let call: Value = serde_json::from_slice(request).unwrap();
        let method = call["method"].as_str().unwrap().to_string();
        self.calls.push(method.clone());
        let attempt = self.attempts.entry(method.clone()).or_default();
        *attempt += 1;
        if self
            .transport_failure
            .as_ref()
            .is_some_and(|(failed_method, failed_attempt)| {
                failed_method == &method && *failed_attempt == *attempt
            })
        {
            return Err(());
        }
        let reply = if method == "state.snapshot" {
            &self.state
        } else {
            &self.terminal
        };
        let result = match reply {
            MethodReply::Valid if method == "state.snapshot" => serde_json::to_value(state_snapshot()).unwrap(),
            MethodReply::Valid => serde_json::to_value(mixed_snapshot()).unwrap(),
            MethodReply::Result(value) => value.clone(),
            MethodReply::RawResult(size) => {
                let raw = format!("\"{}\"", "x".repeat(size.saturating_sub(2)));
                return Ok(format!(
                    "{{\"type\":\"host_response\",\"call_id\":{},\"identity\":{},\"result\":{raw}}}",
                    serde_json::to_string(&call["call_id"]).unwrap(),
                    serde_json::to_string(&call["identity"]).unwrap(),
                ).into_bytes());
            }
            MethodReply::Error => return Ok(serde_json::to_vec(&json!({
                "type": "host_response", "call_id": call["call_id"], "identity": call["identity"],
                "error": {"code": "private", "message": "private"}
            })).unwrap()),
            MethodReply::WrongEnvelope => return Ok(serde_json::to_vec(&json!({
                "type": "wrong", "call_id": call["call_id"], "identity": call["identity"], "result": {}
            })).unwrap()),
            MethodReply::Envelope(value) => {
                let mut value = value.clone();
                if value.get("call_id") == Some(&json!("$call")) {
                    value["call_id"] = call["call_id"].clone();
                }
                if value.get("identity") == Some(&json!("$identity")) {
                    value["identity"] = call["identity"].clone();
                }
                return Ok(serde_json::to_vec(&value).unwrap());
            }
        };
        Ok(serde_json::to_vec(&json!({
            "type": "host_response", "call_id": call["call_id"], "identity": call["identity"], "result": result
        })).unwrap())
    }
}

impl HostBridge for Bridge {
    type Error = ();

    fn cancelled(&mut self) -> bool {
        self.cancellation.pop_front().unwrap_or(false)
    }

    fn call(&mut self, request: &[u8]) -> Result<Vec<u8>, Self::Error> {
        let call: Value = serde_json::from_slice(request).unwrap();
        self.call_bytes.push(request.to_vec());
        self.calls.push(call.clone());
        match &self.reply {
            Reply::Snapshot(snapshot) => {
                let result = if call["method"] == "state.snapshot" {
                    serde_json::to_value(state_snapshot()).unwrap()
                } else {
                    serde_json::to_value(snapshot).unwrap()
                };
                Ok(serde_json::to_vec(&json!({
                    "type": "host_response",
                    "call_id": call["call_id"],
                    "identity": call["identity"],
                    "result": result
                }))
                .unwrap())
            }
            Reply::Envelope(value) => {
                let mut value = value.clone();
                if value.get("call_id") == Some(&json!("$call")) {
                    value["call_id"] = call["call_id"].clone();
                }
                if value.get("identity") == Some(&json!("$identity")) {
                    value["identity"] = call["identity"].clone();
                }
                Ok(serde_json::to_vec(&value).unwrap())
            }
            Reply::DuplicateSnapshot => {
                let identity = serde_json::to_string(&call["identity"]).unwrap();
                let call_id = serde_json::to_string(&call["call_id"]).unwrap();
                Ok(format!(
                    "{{\"type\":\"host_response\",\"call_id\":{call_id},\"identity\":{identity},\"result\":{{\"contract\":\"host.terminals.v1\",\"contract\":\"host.terminals.v1\"}}}}"
                ).into_bytes())
            }
            Reply::Fail => Err(()),
        }
    }
}

fn invoke<B: HostBridge>(bridge: &mut B, request: &Value) -> Result<Value, InvokeError> {
    service::invoke(bridge, &serde_json::to_vec(request).unwrap())
        .map(|bytes| serde_json::from_slice(&bytes).unwrap())
}

fn mixed_snapshot() -> TerminalSnapshot {
    snapshot(vec![
        LayoutItem::Tiled {
            terminal_id: TERM1.into(),
        },
        LayoutItem::Container {
            container_id: "container_0000000000000001".into(),
            terminal_ids: vec![TERM2.into()],
            selected_terminal_id: Some(TERM2.into()),
        },
        LayoutItem::Container {
            container_id: "container_0000000000000002".into(),
            terminal_ids: Vec::new(),
            selected_terminal_id: None,
        },
    ])
}

fn r0_request(method: &str) -> Value {
    let mut request = request_value();
    request["method"] = Value::String(method.into());
    request["params"]["scope"] = json!({"contract": "host.tab.v1", "scope_id": TAB});
    request
}

#[test]
fn r0_profile_and_actions_use_snapshot_workers_not_render_state() {
    let mut profile_bridge = Bridge::new(Reply::Snapshot(mixed_snapshot()));
    let profile = invoke(&mut profile_bridge, &r0_request(crate::r0::PROFILE_METHOD)).unwrap();
    assert_eq!(profile["contract"], crate::r0::PROFILE_CONTRACT);
    assert_eq!(profile["collections"].as_array().unwrap().len(), 2);
    assert_eq!(profile["document"]["roots"].as_array().unwrap().len(), 2);
    assert_eq!(profile_bridge.calls.len(), 2);

    let mut actions_bridge = Bridge::new(Reply::Snapshot(mixed_snapshot()));
    let actions = invoke(&mut actions_bridge, &r0_request(crate::r0::ACTIONS_METHOD)).unwrap();
    assert_eq!(actions["contract"], crate::r0::ACTIONS_CONTRACT);
    assert_eq!(actions["actions"].as_array().unwrap().len(), 3);
    assert_eq!(actions_bridge.calls.len(), 2);
}

#[test]
fn mixed_empty_and_selection_golden() {
    let snapshot = mixed_snapshot();
    let fingerprint = snapshot.fingerprint_sha256.clone();
    assert_eq!(
        fingerprint,
        "ed5988fc573adfda94636fb85ab86064ea55dc802db7abb0185edfe5c86b8fa8"
    );
    let document = project_default(params(), &snapshot).unwrap();
    assert_eq!(
        serde_json::to_value(document).unwrap(),
        json!({
            "contract": "host.document.v2",
            "producer": params().producer,
            "scope": params().scope,
            "revision": 12,
            "dependencies": [{
                "contract": "host.state.v1",
                "scope_id": "state_c44bb5f411b1c3102a74c7fbd1b189ab9f186b39995aeab32dafb8e0e1addc2d",
                "revision": 1,
                "generation": 7
            }, {
                "contract": "host.terminals.v1",
                "scope_id": format!("{TAB}/{fingerprint}"),
                "revision": 1,
                "generation": 5
            }],
            "roots": ["node-000", "node-001", "node-003"],
            "nodes": [
                {"kind":"terminal_slot","id":"node-000","accessibility":{"name":"Terminal"},"terminal_id":TERM1},
                {"kind":"group","id":"node-001","accessibility":{"name":"Collection"},"children":["node-002"]},
                {"kind":"terminal_slot","id":"node-002","accessibility":{"name":"Terminal"},"terminal_id":TERM2},
                {"kind":"group","id":"node-003","accessibility":{"name":"Collection"},"children":[]}
            ]
        })
    );
}

#[test]
fn policy_label_archive_and_stale_ids_preserve_exact_terminal_coverage() {
    let snapshot = snapshot(vec![LayoutItem::Container {
        container_id: "container_0000000000000001".into(),
        terminal_ids: vec![TERM2.into(), TERM1.into()],
        selected_terminal_id: Some(TERM2.into()),
    }]);
    let mut policies = std::collections::BTreeMap::new();
    policies.insert(
        "container_0000000000000001".into(),
        PolicyRecord {
            schema: "vqro.collections.policy".into(),
            schema_version: 1,
            container_id: "container_0000000000000001".into(),
            label: Some(String::new()),
            archived_terminal_ids: vec![
                TERM1.into(),
                "term_00000000000000000000000000000063".into(),
            ],
        },
    );
    let document = project(params(), &state_snapshot(), &policies, &snapshot).unwrap();
    let value = serde_json::to_value(document).unwrap();
    assert_eq!(
        value["nodes"][0]["children"],
        json!(["node-001", "node-002", "node-003"])
    );
    assert_eq!(value["nodes"][1]["text"], "");
    assert_eq!(value["nodes"][2]["terminal_id"], TERM2);
    assert_eq!(value["nodes"][2]["accessibility"]["name"], "Terminal");
    assert_eq!(value["nodes"][3]["terminal_id"], TERM1);
    assert_eq!(
        value["nodes"][3]["accessibility"]["name"],
        "Archived terminal"
    );
    assert!(!serde_json::to_string(&value)
        .unwrap()
        .contains("00000000000000000000000000000063"));
}

#[test]
fn host_state_dependency_vector_and_field_separation_are_exact() {
    let base = StateSnapshot {
        contract: "host.state.v1".into(),
        store_id: "session:0123456789abcdef".into(),
        store_generation: 7,
        namespace: "example.extension".into(),
        revision: 0,
        sequence: 19,
        values: Default::default(),
    };
    let dependency = state_snapshot_dependency(&base).unwrap();
    assert_eq!(
        dependency.scope_id,
        "state_883cb7c92faf86d48599f968d153ec1e1a02336da1db8e622eddb8e3c1fd1f97"
    );
    assert_eq!(dependency.revision, 1);
    assert_eq!(dependency.generation, 7);

    let mut changed_store = base.clone();
    changed_store.store_id.push('0');
    let mut changed_namespace = base.clone();
    changed_namespace.namespace.push('0');
    let mut alternate_boundaries = base.clone();
    alternate_boundaries.store_id = "session:0123456789abcdefe".into();
    alternate_boundaries.namespace = "xample.extension".into();
    let mut changed_revision = base.clone();
    changed_revision.revision = 1;
    let mut changed_generation = base.clone();
    changed_generation.store_generation = 8;
    assert_ne!(
        state_snapshot_dependency(&changed_store).unwrap().scope_id,
        dependency.scope_id
    );
    assert_ne!(
        state_snapshot_dependency(&changed_namespace)
            .unwrap()
            .scope_id,
        dependency.scope_id
    );
    assert_ne!(
        state_snapshot_dependency(&alternate_boundaries)
            .unwrap()
            .scope_id,
        dependency.scope_id,
        "length prefixes bind the store/namespace boundary"
    );
    assert_eq!(
        state_snapshot_dependency(&changed_revision)
            .unwrap()
            .scope_id,
        dependency.scope_id
    );
    assert_eq!(
        state_snapshot_dependency(&changed_revision)
            .unwrap()
            .revision,
        2
    );
    assert_eq!(
        state_snapshot_dependency(&changed_generation)
            .unwrap()
            .scope_id,
        dependency.scope_id
    );
    assert_eq!(
        state_snapshot_dependency(&changed_generation)
            .unwrap()
            .generation,
        8
    );
    let mut non_tuple = base.clone();
    non_tuple.sequence += 1;
    non_tuple.values.insert("other".into(), json!(1));
    assert_eq!(
        state_snapshot_dependency(&non_tuple).unwrap(),
        dependency,
        "sequence and values are not dependency tuple fields"
    );
}

#[test]
fn control_and_aggregate_policy_labels_fail_document_projection() {
    let one_container = snapshot(vec![LayoutItem::Container {
        container_id: "container_0000000000000001".into(),
        terminal_ids: Vec::new(),
        selected_terminal_id: None,
    }]);
    let mut control = std::collections::BTreeMap::new();
    control.insert(
        "container_0000000000000001".into(),
        PolicyRecord {
            schema: "vqro.collections.policy".into(),
            schema_version: 1,
            container_id: "container_0000000000000001".into(),
            label: Some("bad\nlabel".into()),
            archived_terminal_ids: Vec::new(),
        },
    );
    assert_eq!(
        project(params(), &state_snapshot(), &control, &one_container),
        Err(ProjectionError::DocumentInvalid)
    );

    let mut layout = Vec::new();
    let mut aggregate = std::collections::BTreeMap::new();
    for index in 1..=17_u64 {
        let container_id = format!("container_{index:016x}");
        layout.push(LayoutItem::Container {
            container_id: container_id.clone(),
            terminal_ids: Vec::new(),
            selected_terminal_id: None,
        });
        aggregate.insert(
            container_id.clone(),
            PolicyRecord {
                schema: "vqro.collections.policy".into(),
                schema_version: 1,
                container_id,
                label: Some("x".repeat(4096)),
                archived_terminal_ids: Vec::new(),
            },
        );
    }
    assert_eq!(
        project(params(), &state_snapshot(), &aggregate, &snapshot(layout)),
        Err(ProjectionError::DocumentInvalid)
    );
}

#[test]
fn empty_snapshot_is_empty_forest_and_repeat_is_deterministic() {
    let snapshot = snapshot(Vec::new());
    let first = serde_json::to_vec(&project_default(params(), &snapshot).unwrap()).unwrap();
    let second = serde_json::to_vec(&project_default(params(), &snapshot).unwrap()).unwrap();
    assert_eq!(first, second);
    let value: Value = serde_json::from_slice(&first).unwrap();
    assert_eq!(value["roots"], json!([]));
    assert_eq!(value["nodes"], json!([]));
}

#[test]
fn maximum_public_snapshot_projects_with_exact_coverage() {
    let mut layout = Vec::new();
    for index in 0..32 {
        layout.push(LayoutItem::Tiled {
            terminal_id: format!("term_{index:032x}"),
        });
    }
    for index in 0..32 {
        layout.push(LayoutItem::Container {
            container_id: format!("container_{index:016x}"),
            terminal_ids: Vec::new(),
            selected_terminal_id: None,
        });
    }
    let document = project_default(params(), &snapshot(layout)).unwrap();
    assert_eq!(document.roots.len(), 64);
    assert_eq!(document.nodes.len(), 64);
}

#[test]
fn service_uses_two_exact_read_calls_and_emits_no_forbidden_fields() {
    let mut bridge = Bridge::new(Reply::Snapshot(mixed_snapshot()));
    let output = invoke(&mut bridge, &request_value()).unwrap();
    assert_eq!(bridge.calls.len(), 2);
    for call in &bridge.calls {
        assert_eq!(call["type"], "host_call");
        assert_eq!(call["depth"], 1);
        assert_eq!(call["namespace"], "vqro.collections");
        assert_eq!(call["identity"], request_value()["identity"]);
        assert!(call["call_id"].as_str().unwrap().len() <= 128);
    }
    assert_eq!(bridge.calls[0]["capability"], "host.state.read");
    assert_eq!(bridge.calls[0]["method"], "state.snapshot");
    assert_eq!(
        bridge.calls[0]["params"],
        json!({"contract":"host.state.v1"})
    );
    assert_eq!(bridge.calls[1]["capability"], "host.terminals.read");
    assert_eq!(bridge.calls[1]["method"], "terminals.snapshot");
    assert_eq!(
        bridge.calls[1]["params"],
        json!({"contract":"host.terminals.v1"})
    );
    let encoded = serde_json::to_string(&output).unwrap();
    for forbidden in [
        "PaneId", "pane_id", "geometry", "archive", "label", "action", "effect", "selected",
    ] {
        assert!(!encoded.contains(forbidden), "found {forbidden}");
    }
}

#[test]
fn wit_boundary_bytes_use_recursive_canonical_object_order() {
    let request = request_value();
    let request_bytes = serde_json::to_vec(&request).unwrap();
    let digest = format!("{:x}", sha2::Sha256::digest(&request_bytes));
    let state_call_id = format!("collections-state-{digest}");
    let terminal_call_id = format!("collections-terminals-{digest}");
    let snapshot = mixed_snapshot();
    let fingerprint = snapshot.fingerprint_sha256.clone();
    let mut bridge = Bridge::new(Reply::Snapshot(snapshot));

    let document_bytes = service::invoke(&mut bridge, &request_bytes).unwrap();

    assert_eq!(
        bridge.call_bytes[0],
        format!(
            "{{\"call_id\":\"{state_call_id}\",\"capability\":\"host.state.read\",\"depth\":1,\"identity\":{{\"generation\":9,\"namespace\":\"vqro.collections\",\"package_id\":\"vqro.collections\",\"service_id\":\"collections\"}},\"method\":\"state.snapshot\",\"namespace\":\"vqro.collections\",\"params\":{{\"contract\":\"host.state.v1\"}},\"type\":\"host_call\"}}"
        )
        .into_bytes()
    );
    assert_eq!(
        bridge.call_bytes[1],
        format!(
            "{{\"call_id\":\"{terminal_call_id}\",\"capability\":\"host.terminals.read\",\"depth\":1,\"identity\":{{\"generation\":9,\"namespace\":\"vqro.collections\",\"package_id\":\"vqro.collections\",\"service_id\":\"collections\"}},\"method\":\"terminals.snapshot\",\"namespace\":\"vqro.collections\",\"params\":{{\"contract\":\"host.terminals.v1\"}},\"type\":\"host_call\"}}"
        ).into_bytes()
    );
    let value: Value = serde_json::from_slice(&document_bytes).unwrap();
    assert_eq!(value["dependencies"].as_array().unwrap().len(), 2);
    assert_eq!(
        value["dependencies"][1]["scope_id"],
        format!("{TAB}/{fingerprint}")
    );
}

#[test]
fn all_outer_identity_contract_and_fence_errors_make_zero_calls() {
    let cases = [
        ("contract", json!("wrong")),
        ("method", json!("wrong")),
        ("identity.package_id", json!("other.package")),
        ("identity.namespace", json!("other.namespace")),
        ("identity.service_id", json!("other")),
        ("identity.generation", json!(0)),
        ("params.contract", json!("wrong")),
        ("params.producer.package_id", json!("other.package")),
        ("params.producer.service_id", json!("other")),
        ("params.producer.runtime_generation", json!(8)),
        ("params.producer.provider_generation", json!(0)),
        ("params.producer.scope_generation", json!(0)),
        ("params.requested_revision", json!(0)),
    ];
    for (path, replacement) in cases {
        let mut request = request_value();
        set_path(&mut request, path, replacement);
        let mut bridge = Bridge::new(Reply::Snapshot(mixed_snapshot()));
        assert_eq!(
            invoke(&mut bridge, &request),
            Err(InvokeError::InvalidRequest),
            "{path}"
        );
        assert!(bridge.calls.is_empty(), "{path}");
    }
}

#[test]
fn unknown_request_and_params_fields_are_rejected_without_call() {
    for path in [
        "unknown",
        "params.unknown",
        "params.producer.unknown",
        "params.scope.unknown",
    ] {
        let mut request = request_value();
        insert_path(&mut request, path, json!(true));
        let mut bridge = Bridge::new(Reply::Snapshot(mixed_snapshot()));
        assert_eq!(
            invoke(&mut bridge, &request),
            Err(InvokeError::InvalidRequest)
        );
        assert!(bridge.calls.is_empty());
    }
}

#[test]
fn envelope_contract_identity_and_shape_errors_stop_after_one_call() {
    let good_identity = "$identity";
    let cases = vec![
        json!({"type":"host_call_response","call_id":"$call","identity":good_identity,"result":mixed_snapshot()}),
        json!({"type":"wrong","call_id":"$call","identity":good_identity,"result":mixed_snapshot()}),
        json!({"type":"host_response","call_id":"wrong","identity":good_identity,"result":mixed_snapshot()}),
        json!({"type":"host_response","call_id":"$call","identity":{"package_id":"vqro.collections","namespace":"vqro.collections","service_id":"collections","generation":8},"result":mixed_snapshot()}),
        json!({"type":"host_response","call_id":"$call","identity":good_identity}),
        json!({"type":"host_response","call_id":"$call","identity":good_identity,"result":mixed_snapshot(),"error":{"code":"x","message":"secret"}}),
        json!({"type":"host_response","call_id":"$call","identity":good_identity,"result":mixed_snapshot(),"unknown":true}),
    ];
    for envelope in cases {
        let mut bridge = Bridge::new(Reply::Envelope(envelope));
        assert_eq!(
            invoke(&mut bridge, &request_value()),
            Err(InvokeError::InvalidHostResponse)
        );
        assert_eq!(bridge.calls.len(), 1);
    }
}

#[test]
fn accepted_host_error_is_static_and_not_leaked() {
    let envelope = json!({
        "type":"host_response", "call_id":"$call", "identity":"$identity",
        "error":{"code":"host_secret_code","message":"payload secret"}
    });
    let mut bridge = Bridge::new(Reply::Envelope(envelope));
    let error =
        service::invoke(&mut bridge, &serde_json::to_vec(&request_value()).unwrap()).unwrap_err();
    assert_eq!(error, InvokeError::HostCallFailed);
    assert_eq!(error.message(), "host call failed");
    assert!(!error.message().contains("secret"));
}

#[test]
fn snapshot_contract_unknown_fingerprint_and_topology_errors_are_rejected() {
    let mut snapshots = Vec::new();
    let mut wrong_contract = mixed_snapshot();
    wrong_contract.contract = "wrong".into();
    snapshots.push(wrong_contract);
    let mut wrong_fingerprint = mixed_snapshot();
    wrong_fingerprint.fingerprint_sha256 = "f".repeat(64);
    snapshots.push(wrong_fingerprint);
    snapshots.push(snapshot(vec![
        LayoutItem::Tiled {
            terminal_id: TERM1.into(),
        },
        LayoutItem::Tiled {
            terminal_id: TERM1.into(),
        },
    ]));
    snapshots.push(snapshot(vec![LayoutItem::Container {
        container_id: "container_0000000000000001".into(),
        terminal_ids: vec![TERM1.into()],
        selected_terminal_id: Some(TERM2.into()),
    }]));
    for snapshot in snapshots {
        let mut bridge = Bridge::new(Reply::Snapshot(snapshot));
        assert_eq!(
            invoke(&mut bridge, &request_value()),
            Err(InvokeError::InvalidSnapshot)
        );
        assert_eq!(bridge.calls.len(), 2);
    }

    let mut value = serde_json::to_value(mixed_snapshot()).unwrap();
    value["unknown"] = json!(true);
    let envelope =
        json!({"type":"host_response","call_id":"$call","identity":"$identity","result":value});
    let mut bridge = Bridge::new(Reply::Envelope(envelope));
    assert_eq!(
        invoke(&mut bridge, &request_value()),
        Err(InvokeError::InvalidSnapshot)
    );
}

#[test]
fn snapshot_bounds_are_enforced() {
    let mut too_many_layout = Vec::new();
    for index in 0..65 {
        too_many_layout.push(LayoutItem::Container {
            container_id: format!("container_{index:016x}"),
            terminal_ids: Vec::new(),
            selected_terminal_id: None,
        });
    }
    assert_eq!(
        validate_snapshot(&snapshot(too_many_layout)),
        Err(ProjectionError::SnapshotInvalid)
    );

    let members = (0..33).map(|i| format!("term_{i:032x}")).collect();
    let value = snapshot(vec![LayoutItem::Container {
        container_id: "container_0000000000000001".into(),
        terminal_ids: members,
        selected_terminal_id: None,
    }]);
    assert_eq!(
        validate_snapshot(&value),
        Err(ProjectionError::SnapshotInvalid)
    );
}

#[test]
fn host_snapshot_semantic_boundaries_match_normative_validator() {
    let mut missing_selection = snapshot(vec![LayoutItem::Container {
        container_id: "container_0000000000000001".into(),
        terminal_ids: vec![TERM1.into()],
        selected_terminal_id: None,
    }]);
    missing_selection.fingerprint_sha256 = canonical_fingerprint(&missing_selection).unwrap();
    assert!(validate_snapshot(&missing_selection).is_err());

    let mut selected_empty = snapshot(vec![LayoutItem::Container {
        container_id: "container_0000000000000001".into(),
        terminal_ids: Vec::new(),
        selected_terminal_id: Some(TERM1.into()),
    }]);
    selected_empty.fingerprint_sha256 = canonical_fingerprint(&selected_empty).unwrap();
    assert!(validate_snapshot(&selected_empty).is_err());

    let thirty_three_terminals = snapshot(
        (0..33)
            .map(|index| LayoutItem::Tiled {
                terminal_id: format!("term_{index:032x}"),
            })
            .collect(),
    );
    assert!(validate_snapshot(&thirty_three_terminals).is_err());

    let thirty_three_containers = snapshot(
        (0..33)
            .map(|index| LayoutItem::Container {
                container_id: format!("container_{index:016x}"),
                terminal_ids: Vec::new(),
                selected_terminal_id: None,
            })
            .collect(),
    );
    assert!(validate_snapshot(&thirty_three_containers).is_err());

    for mutation in [
        ("workspace_id", json!("w1111111111111111111111111111111")),
        ("tab_id", json!("tab_0000000000000000000000000000000A")),
    ] {
        let mut value = serde_json::to_value(snapshot(Vec::new())).unwrap();
        value[mutation.0] = mutation.1;
        let snapshot: TerminalSnapshot = serde_json::from_value(value).unwrap();
        assert!(validate_snapshot(&snapshot).is_err());
    }
}

#[test]
fn method_sensitive_state_and_terminal_failures_stop_without_retry() {
    let state_cases = [
        (MethodReply::Result(json!({})), InvokeError::InvalidSnapshot),
        (MethodReply::Error, InvokeError::HostCallFailed),
        (MethodReply::WrongEnvelope, InvokeError::InvalidHostResponse),
        (
            MethodReply::RawResult(MAX_STATE_SNAPSHOT_BYTES),
            InvokeError::InvalidSnapshot,
        ),
        (
            MethodReply::RawResult(MAX_STATE_SNAPSHOT_BYTES + 1),
            InvokeError::InvalidHostResponse,
        ),
        (
            MethodReply::RawResult(262_145),
            InvokeError::InvalidHostResponse,
        ),
    ];
    for (reply, expected) in state_cases {
        let mut bridge = MethodBridge::new(reply, MethodReply::Valid);
        assert_eq!(invoke(&mut bridge, &request_value()), Err(expected));
        assert_eq!(bridge.calls, ["state.snapshot"]);
    }

    let terminal_cases = [
        (MethodReply::Result(json!({})), InvokeError::InvalidSnapshot),
        (MethodReply::Error, InvokeError::HostCallFailed),
        (MethodReply::WrongEnvelope, InvokeError::InvalidHostResponse),
        (
            MethodReply::RawResult(MAX_SNAPSHOT_BYTES),
            InvokeError::InvalidSnapshot,
        ),
        (
            MethodReply::RawResult(MAX_SNAPSHOT_BYTES + 1),
            InvokeError::InvalidHostResponse,
        ),
        (
            MethodReply::RawResult(262_145),
            InvokeError::InvalidHostResponse,
        ),
    ];
    for (reply, expected) in terminal_cases {
        let mut bridge = MethodBridge::new(MethodReply::Valid, reply);
        assert_eq!(invoke(&mut bridge, &request_value()), Err(expected));
        assert_eq!(bridge.calls, ["state.snapshot", "terminals.snapshot"]);
    }
}

#[test]
fn every_state_snapshot_identity_and_shape_error_stops_before_terminals() {
    let valid = serde_json::to_value(state_snapshot()).unwrap();
    let mutations = [
        ("contract", json!("wrong")),
        ("namespace", json!("other.namespace")),
        ("store_id", json!("")),
        ("store_id", json!("bad/id")),
        ("store_generation", json!(0)),
        ("revision", json!(u64::MAX)),
    ];
    for (field, replacement) in mutations {
        let mut result = valid.clone();
        result[field] = replacement;
        let mut bridge = MethodBridge::new(MethodReply::Result(result), MethodReply::Valid);
        assert_eq!(
            invoke(&mut bridge, &request_value()),
            Err(InvokeError::InvalidSnapshot)
        );
        assert_eq!(bridge.calls, ["state.snapshot"]);
    }
    let mut unknown = valid;
    unknown["unknown"] = json!(true);
    let mut bridge = MethodBridge::new(MethodReply::Result(unknown), MethodReply::Valid);
    assert_eq!(
        invoke(&mut bridge, &request_value()),
        Err(InvokeError::InvalidSnapshot)
    );
    assert_eq!(bridge.calls, ["state.snapshot"]);
}

#[test]
fn every_malformed_envelope_is_checked_at_both_methods() {
    let malformed = vec![
        json!({"type":"host_call_response","call_id":"$call","identity":"$identity","result":{}}),
        json!({"type":"host_response","call_id":"wrong","identity":"$identity","result":{}}),
        json!({"type":"host_response","call_id":"$call","identity":{"package_id":"vqro.collections","namespace":"vqro.collections","service_id":"collections","generation":8},"result":{}}),
        json!({"type":"host_response","call_id":"$call","identity":"$identity"}),
        json!({"type":"host_response","call_id":"$call","identity":"$identity","result":{},"error":{"code":"x","message":"x"}}),
        json!({"type":"host_response","call_id":"$call","identity":"$identity","result":{},"unknown":true}),
    ];
    for envelope in &malformed {
        let mut state =
            MethodBridge::new(MethodReply::Envelope(envelope.clone()), MethodReply::Valid);
        assert_eq!(
            invoke(&mut state, &request_value()),
            Err(InvokeError::InvalidHostResponse)
        );
        assert_eq!(state.calls, ["state.snapshot"]);

        let mut terminal =
            MethodBridge::new(MethodReply::Valid, MethodReply::Envelope(envelope.clone()));
        assert_eq!(
            invoke(&mut terminal, &request_value()),
            Err(InvokeError::InvalidHostResponse)
        );
        assert_eq!(terminal.calls, ["state.snapshot", "terminals.snapshot"]);
    }
}

#[test]
fn terminal_transport_failure_after_successful_state_has_exact_call_order() {
    let mut bridge = MethodBridge::new(MethodReply::Valid, MethodReply::Valid)
        .fail_transport("terminals.snapshot", 1);
    assert_eq!(
        invoke(&mut bridge, &request_value()),
        Err(InvokeError::HostCallFailed)
    );
    assert_eq!(bridge.calls, ["state.snapshot", "terminals.snapshot"]);
}

#[test]
fn cancellation_wins_after_failed_terminal_transport_attempt() {
    let mut bridge = MethodBridge::new(MethodReply::Valid, MethodReply::Valid)
        .fail_transport("terminals.snapshot", 1);
    bridge.cancellation = VecDeque::from([false, false, false, true]);
    assert_eq!(
        invoke(&mut bridge, &request_value()),
        Err(InvokeError::Cancelled)
    );
    assert_eq!(bridge.calls, ["state.snapshot", "terminals.snapshot"]);
}

#[test]
fn method_sensitive_cancellation_checks_cover_every_call_boundary() {
    let cases = [
        (VecDeque::from([true]), 0),
        (VecDeque::from([false, true]), 1),
        (VecDeque::from([false, false, true]), 1),
        (VecDeque::from([false, false, false, true]), 2),
    ];
    for (cancellation, expected_calls) in cases {
        let mut bridge = MethodBridge::new(MethodReply::Valid, MethodReply::Valid);
        bridge.cancellation = cancellation;
        assert_eq!(
            invoke(&mut bridge, &request_value()),
            Err(InvokeError::Cancelled)
        );
        assert_eq!(bridge.calls.len(), expected_calls);
    }
}

#[test]
fn cancellation_and_bridge_failure_never_retry() {
    let mut before = Bridge::new(Reply::Snapshot(mixed_snapshot()));
    before.cancellation = VecDeque::from([true]);
    assert_eq!(
        invoke(&mut before, &request_value()),
        Err(InvokeError::Cancelled)
    );
    assert!(before.calls.is_empty());

    let mut after = Bridge::new(Reply::Snapshot(mixed_snapshot()));
    after.cancellation = VecDeque::from([false, true]);
    assert_eq!(
        invoke(&mut after, &request_value()),
        Err(InvokeError::Cancelled)
    );
    assert_eq!(after.calls.len(), 1);

    let mut failed = Bridge::new(Reply::Fail);
    assert_eq!(
        invoke(&mut failed, &request_value()),
        Err(InvokeError::HostCallFailed)
    );
    assert_eq!(failed.calls.len(), 1);
}

#[test]
fn document_validator_rejects_graph_coverage_control_and_dependency_errors() {
    let snapshot = mixed_snapshot();
    let terminals = validate_snapshot(&snapshot).unwrap();
    let baseline = project_default(params(), &snapshot).unwrap();

    let mut unreachable = baseline.clone();
    unreachable.roots.pop();
    assert_eq!(
        validate_document(&unreachable, &terminals, true),
        Err(ProjectionError::DocumentInvalid)
    );

    let mut duplicate_parent = baseline.clone();
    if let DocumentNode::Group { children, .. } = &mut duplicate_parent.nodes[1] {
        children.push("node-000".into());
    }
    assert_eq!(
        validate_document(&duplicate_parent, &terminals, true),
        Err(ProjectionError::DocumentInvalid)
    );

    let mut control = baseline.clone();
    if let DocumentNode::TerminalSlot { accessibility, .. } = &mut control.nodes[0] {
        accessibility.name = "bad\u{1b}".into();
    }
    assert_eq!(
        validate_document(&control, &terminals, true),
        Err(ProjectionError::DocumentInvalid)
    );

    let mut missing = baseline.clone();
    missing.nodes.retain(|node| !matches!(node, DocumentNode::TerminalSlot { terminal_id, .. } if terminal_id == TERM1));
    missing.roots.retain(|root| root != "node-000");
    assert_eq!(
        validate_document(&missing, &terminals, true),
        Err(ProjectionError::DocumentInvalid)
    );

    let mut dependencies = baseline;
    dependencies
        .dependencies
        .push(dependencies.dependencies[0].clone());
    assert_eq!(
        validate_document(&dependencies, &terminals, true),
        Err(ProjectionError::DocumentInvalid)
    );
}

#[test]
fn oversized_input_and_request_id_are_rejected_before_call() {
    let mut request = request_value();
    request["request_id"] = json!("x".repeat(129));
    let mut bridge = Bridge::new(Reply::Snapshot(mixed_snapshot()));
    assert_eq!(
        invoke(&mut bridge, &request),
        Err(InvokeError::InvalidRequest)
    );
    assert!(bridge.calls.is_empty());

    let bytes = vec![b' '; 262_145];
    assert_eq!(
        service::invoke(&mut bridge, &bytes),
        Err(InvokeError::InvalidRequest)
    );
    assert!(bridge.calls.is_empty());
}

#[test]
fn host_golden_document_fixtures_match_our_dto_and_validator() {
    let valid: HostDocument = serde_json::from_slice(include_bytes!(
        "../../../contracts/fixtures/host-document-v2/valid.json"
    ))
    .unwrap();
    let allowed = std::collections::BTreeSet::from([TERM1.to_string()]);
    validate_document(&valid, &allowed, true).unwrap();

    for invalid in [
        include_bytes!("../../../contracts/fixtures/host-document-v2/invalid-action.json")
            .as_slice(),
        include_bytes!("../../../contracts/fixtures/host-document-v2/invalid-unknown-field.json")
            .as_slice(),
    ] {
        assert!(serde_json::from_slice::<HostDocument>(invalid).is_err());
    }
    let unreachable: HostDocument = serde_json::from_slice(include_bytes!(
        "../../../contracts/fixtures/host-document-v2/invalid-unreachable.json"
    ))
    .unwrap();
    assert_eq!(
        validate_document(&unreachable, &std::collections::BTreeSet::new(), false),
        Err(ProjectionError::DocumentInvalid)
    );
}

#[test]
fn shared_host_fingerprint_vector_is_exact() {
    let vector: Value = serde_json::from_slice(include_bytes!(
        "../../../contracts/fixtures/host-terminals-v1/fingerprint-vector.json"
    ))
    .unwrap();
    let snapshot: TerminalSnapshot = serde_json::from_value(vector["snapshot"].clone()).unwrap();
    let expected = vector["fingerprint_sha256"].as_str().unwrap();
    assert_eq!(canonical_fingerprint(&snapshot).unwrap(), expected);
    assert_eq!(snapshot.fingerprint_sha256, expected);
    validate_snapshot(&snapshot).unwrap();

    assert_eq!(
        String::from_utf8(canonical_fingerprint_input(&snapshot).unwrap()).unwrap(),
        vector["canonical_input_utf8"].as_str().unwrap()
    );
}

#[test]
fn duplicate_json_members_are_rejected_on_reachable_paths() {
    let encoded = serde_json::to_string(&request_value()).unwrap();
    let duplicate_outer = encoded.replacen("{", "{\"contract\":\"vqro.service.v1\",", 1);
    let mut bridge = Bridge::new(Reply::Snapshot(mixed_snapshot()));
    assert_eq!(
        service::invoke(&mut bridge, duplicate_outer.as_bytes()),
        Err(InvokeError::InvalidRequest)
    );
    assert!(bridge.calls.is_empty());

    let duplicate_params = encoded.replacen(
        "\"params\":{",
        "\"params\":{\"contract\":\"host.document.render.v2\",",
        1,
    );
    assert_eq!(
        service::invoke(&mut bridge, duplicate_params.as_bytes()),
        Err(InvokeError::InvalidRequest)
    );
    assert!(bridge.calls.is_empty());

    let mut duplicate_snapshot = Bridge::new(Reply::DuplicateSnapshot);
    assert_eq!(
        invoke(&mut duplicate_snapshot, &request_value()),
        Err(InvokeError::InvalidSnapshot)
    );
    assert_eq!(duplicate_snapshot.calls.len(), 1);
}

#[test]
fn unicode_limits_count_characters_not_utf8_bytes() {
    let mut document = project_default(params(), &snapshot(Vec::new())).unwrap();
    document.roots = vec!["unicode".into()];
    document.nodes = vec![DocumentNode::Text {
        id: "unicode".into(),
        accessibility: Accessibility {
            name: "🦀".repeat(256),
            description: Some("é".repeat(1024)),
        },
        text: "界".repeat(4096),
    }];
    validate_document(&document, &std::collections::BTreeSet::new(), true).unwrap();

    if let DocumentNode::Text { accessibility, .. } = &mut document.nodes[0] {
        accessibility.name.push('🦀');
    }
    assert_eq!(
        validate_document(&document, &std::collections::BTreeSet::new(), true),
        Err(ProjectionError::DocumentInvalid)
    );
}

#[test]
fn u64_extremes_and_every_zero_fence_are_checked_through_service() {
    let mut minimum_request = request_value();
    minimum_request["identity"]["generation"] = json!(1);
    minimum_request["params"]["producer"]["runtime_generation"] = json!(1);
    minimum_request["params"]["producer"]["provider_generation"] = json!(1);
    minimum_request["params"]["producer"]["scope_generation"] = json!(1);
    minimum_request["params"]["requested_revision"] = json!(1);
    let mut minimum_snapshot = snapshot(Vec::new());
    minimum_snapshot.lease_fence = LeaseFence {
        catalog_generation: 1,
        package_generation: 1,
        service_generation: 1,
        artifact_generation: 1,
        provider_generation: 1,
        runtime_generation: 1,
    };
    minimum_snapshot.fingerprint_sha256 = canonical_fingerprint(&minimum_snapshot).unwrap();
    let mut minimum_bridge = Bridge::new(Reply::Snapshot(minimum_snapshot));
    assert!(invoke(&mut minimum_bridge, &minimum_request).is_ok());

    let mut request = request_value();
    request["identity"]["generation"] = json!(u64::MAX);
    request["params"]["producer"]["runtime_generation"] = json!(u64::MAX);
    request["params"]["producer"]["provider_generation"] = json!(u64::MAX);
    request["params"]["producer"]["scope_generation"] = json!(u64::MAX);
    request["params"]["requested_revision"] = json!(u64::MAX);
    let mut maximum_snapshot = snapshot(Vec::new());
    maximum_snapshot.lease_fence = LeaseFence {
        catalog_generation: u64::MAX,
        package_generation: u64::MAX,
        service_generation: u64::MAX,
        artifact_generation: u64::MAX,
        provider_generation: u64::MAX,
        runtime_generation: u64::MAX,
    };
    maximum_snapshot.fingerprint_sha256 = canonical_fingerprint(&maximum_snapshot).unwrap();
    let mut bridge = Bridge::new(Reply::Snapshot(maximum_snapshot));
    assert!(invoke(&mut bridge, &request).is_ok());
    assert_eq!(bridge.calls.len(), 2);

    for field in [
        "catalog_generation",
        "package_generation",
        "service_generation",
        "artifact_generation",
        "provider_generation",
        "runtime_generation",
    ] {
        let mut value = serde_json::to_value(snapshot(Vec::new())).unwrap();
        value["lease_fence"][field] = json!(0);
        value["fingerprint_sha256"] = json!("0".repeat(64));
        let invalid: TerminalSnapshot = serde_json::from_value(value).unwrap();
        let mut bridge = Bridge::new(Reply::Snapshot(invalid));
        assert_eq!(
            invoke(&mut bridge, &request_value()),
            Err(InvokeError::InvalidSnapshot),
            "accepted zero {field}"
        );
    }
}

#[test]
fn encoded_limits_are_exact_on_service_and_document_paths() {
    let base = serde_json::to_string(&params()).unwrap();
    let params_2k = format!(
        "{}{}{}",
        &base[..base.len() - 1],
        " ".repeat(2048 - base.len()),
        "}"
    );
    assert_eq!(params_2k.len(), 2048);
    let request_2k = request_with_raw_params(&params_2k);
    let mut bridge = Bridge::new(Reply::Snapshot(snapshot(Vec::new())));
    assert!(service::invoke(&mut bridge, request_2k.as_bytes()).is_ok());
    assert_eq!(bridge.calls.len(), 2);

    let params_over = format!("{} {}", &params_2k[..params_2k.len() - 1], "}");
    assert_eq!(params_over.len(), 2049);
    let mut bridge = Bridge::new(Reply::Snapshot(snapshot(Vec::new())));
    assert_eq!(
        service::invoke(
            &mut bridge,
            request_with_raw_params(&params_over).as_bytes()
        ),
        Err(InvokeError::InvalidRequest)
    );
    assert!(bridge.calls.is_empty());

    let request = serde_json::to_vec(&request_value()).unwrap();
    let mut frame_at_limit = request.clone();
    frame_at_limit.resize(262_144, b' ');
    let mut bridge = Bridge::new(Reply::Snapshot(snapshot(Vec::new())));
    assert!(service::invoke(&mut bridge, &frame_at_limit).is_ok());
    frame_at_limit.push(b' ');
    let mut bridge = Bridge::new(Reply::Snapshot(snapshot(Vec::new())));
    assert_eq!(
        service::invoke(&mut bridge, &frame_at_limit),
        Err(InvokeError::InvalidRequest)
    );
    assert!(bridge.calls.is_empty());

    let mut exact = SizedResultBridge::new(65_536);
    assert_eq!(
        service::invoke(&mut exact, &request),
        Err(InvokeError::InvalidSnapshot)
    );
    let mut over = SizedResultBridge::new(65_537);
    assert_eq!(
        service::invoke(&mut over, &request),
        Err(InvokeError::InvalidSnapshot)
    );

    let mut document = maximum_encoded_document();
    let allowed = std::collections::BTreeSet::new();
    assert_eq!(serde_json::to_vec(&document).unwrap().len(), 131_072);
    validate_document(&document, &allowed, true).unwrap();
    for node in &mut document.nodes {
        if let DocumentNode::Text { text, .. } = node {
            if let Some(index) = text.find('x') {
                text.replace_range(index..index + 1, "é");
                break;
            }
        }
    }
    assert_eq!(serde_json::to_vec(&document).unwrap().len(), 131_073);
    assert_eq!(
        validate_document(&document, &allowed, true),
        Err(ProjectionError::DocumentInvalid)
    );
}

#[test]
fn document_collection_and_integer_boundaries_are_exact() {
    let allowed = std::collections::BTreeSet::new();
    let mut document = project_default(params(), &snapshot(Vec::new())).unwrap();
    document.revision = u64::MAX;
    document.dependencies = (0..64)
        .map(|index| DocumentDependency {
            contract: "host.scope.v1".into(),
            scope_id: format!("s{index:03}"),
            revision: u64::MAX,
            generation: u64::MAX,
        })
        .collect();
    document.roots = (0..512).map(|index| format!("n{index}")).collect();
    document.nodes = (0..512)
        .map(|index| DocumentNode::Text {
            id: format!("n{index}"),
            accessibility: Accessibility {
                name: "N".into(),
                description: None,
            },
            text: String::new(),
        })
        .collect();
    validate_document(&document, &allowed, true).unwrap();

    let mut too_many_dependencies = document.clone();
    too_many_dependencies.dependencies.push(DocumentDependency {
        contract: "host.scope.v1".into(),
        scope_id: "s999".into(),
        revision: 1,
        generation: 1,
    });
    assert!(validate_document(&too_many_dependencies, &allowed, true).is_err());

    let mut too_many_nodes = document.clone();
    too_many_nodes.roots.push("n512".into());
    too_many_nodes.nodes.push(DocumentNode::Text {
        id: "n512".into(),
        accessibility: Accessibility {
            name: "N".into(),
            description: None,
        },
        text: String::new(),
    });
    assert!(validate_document(&too_many_nodes, &allowed, true).is_err());

    for field in ["revision", "dependency_revision", "dependency_generation"] {
        let mut zero = document.clone();
        match field {
            "revision" => zero.revision = 0,
            "dependency_revision" => zero.dependencies[0].revision = 0,
            "dependency_generation" => zero.dependencies[0].generation = 0,
            _ => unreachable!(),
        }
        assert!(
            validate_document(&zero, &allowed, true).is_err(),
            "accepted {field}"
        );
    }
}

#[test]
fn graph_cycle_depth_missing_reference_and_ownership_reject_through_validator() {
    let allowed = std::collections::BTreeSet::new();
    let base = |nodes, roots| HostDocument {
        contract: "host.document.v2".into(),
        producer: params().producer,
        scope: params().scope,
        revision: 1,
        dependencies: Vec::new(),
        roots,
        nodes,
    };
    let access = || Accessibility {
        name: "G".into(),
        description: None,
    };

    let cycle = base(
        vec![
            DocumentNode::Group {
                id: "a".into(),
                accessibility: access(),
                children: vec!["b".into()],
            },
            DocumentNode::Group {
                id: "b".into(),
                accessibility: access(),
                children: vec!["a".into()],
            },
        ],
        Vec::new(),
    );
    assert!(validate_document(&cycle, &allowed, true).is_err());

    let depth_nodes = (0..17)
        .map(|index| DocumentNode::Group {
            id: format!("n{index}"),
            accessibility: access(),
            children: if index == 16 {
                Vec::new()
            } else {
                vec![format!("n{}", index + 1)]
            },
        })
        .collect();
    assert!(validate_document(&base(depth_nodes, vec!["n0".into()]), &allowed, true).is_err());

    let missing = base(
        vec![DocumentNode::Group {
            id: "root".into(),
            accessibility: access(),
            children: vec!["missing".into()],
        }],
        vec!["root".into()],
    );
    assert!(validate_document(&missing, &allowed, true).is_err());

    let owned_twice = base(
        vec![
            DocumentNode::Group {
                id: "a".into(),
                accessibility: access(),
                children: vec!["child".into()],
            },
            DocumentNode::Group {
                id: "b".into(),
                accessibility: access(),
                children: vec!["child".into()],
            },
            DocumentNode::Text {
                id: "child".into(),
                accessibility: access(),
                text: String::new(),
            },
        ],
        vec!["a".into(), "b".into()],
    );
    assert!(validate_document(&owned_twice, &allowed, true).is_err());
}

fn request_with_raw_params(params: &str) -> String {
    let mut value = request_value();
    value.as_object_mut().unwrap().remove("params");
    let prefix = serde_json::to_string(&value).unwrap();
    format!("{{\"params\":{params},{}", &prefix[1..])
}

struct SizedResultBridge {
    target: usize,
    calls: usize,
}

impl SizedResultBridge {
    fn new(target: usize) -> Self {
        Self { target, calls: 0 }
    }
}

impl HostBridge for SizedResultBridge {
    type Error = ();

    fn cancelled(&mut self) -> bool {
        false
    }

    fn call(&mut self, request: &[u8]) -> Result<Vec<u8>, Self::Error> {
        self.calls += 1;
        let call: Value = serde_json::from_slice(request).unwrap();
        let result = format!("\"{}\"", "x".repeat(self.target - 2));
        assert_eq!(result.len(), self.target);
        Ok(format!(
            "{{\"type\":\"host_response\",\"call_id\":{},\"identity\":{},\"result\":{result}}}",
            serde_json::to_string(&call["call_id"]).unwrap(),
            serde_json::to_string(&call["identity"]).unwrap()
        )
        .into_bytes())
    }
}

fn maximum_encoded_document() -> HostDocument {
    let mut document = HostDocument {
        contract: "host.document.v2".into(),
        producer: params().producer,
        scope: params().scope,
        revision: 1,
        dependencies: Vec::new(),
        roots: (0..16).map(|index| format!("n{index}")).collect(),
        nodes: (0..16)
            .map(|index| DocumentNode::Text {
                id: format!("n{index}"),
                accessibility: Accessibility {
                    name: "N".into(),
                    description: None,
                },
                text: "x".repeat(4095),
            })
            .collect(),
    };
    let baseline = serde_json::to_vec(&document).unwrap().len();
    let extra = 131_072 - baseline;
    assert!(extra <= 16 * 4095);
    let mut remaining = extra;
    for node in &mut document.nodes {
        if let DocumentNode::Text { text, .. } = node {
            let replacements = remaining.min(text.len());
            *text = format!(
                "{}{}",
                "é".repeat(replacements),
                "x".repeat(text.len() - replacements)
            );
            remaining -= replacements;
        }
    }
    assert_eq!(remaining, 0);
    document
}

fn set_path(root: &mut Value, path: &str, value: Value) {
    let mut current = root;
    let mut parts = path.split('.').peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            current[part] = value;
            return;
        }
        current = &mut current[part];
    }
}

fn insert_path(root: &mut Value, path: &str, value: Value) {
    set_path(root, path, value);
}
