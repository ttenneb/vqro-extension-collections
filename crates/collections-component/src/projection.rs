//! Pure, authority-free terminal snapshot validation and document projection.

use crate::policy::{archived, validate_neutral, PolicyRecord};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_SNAPSHOT_BYTES: usize = 65_536;
pub const MAX_STATE_SNAPSHOT_BYTES: usize = 196_608;
pub const MAX_DOCUMENT_BYTES: usize = 131_072;
const MAX_LAYOUT_ITEMS: usize = 64;
const MAX_TERMINALS: usize = 32;
const MAX_CONTAINERS: usize = 32;
const MAX_MEMBERS: usize = 64;
const MAX_NODES: usize = 512;
const MAX_DEPTH: usize = 16;
const MAX_TOTAL_TEXT_CHARS: usize = 65_536;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProducerFence {
    pub package_id: String,
    pub service_id: String,
    pub artifact_sha256: String,
    pub runtime_generation: u64,
    pub provider_generation: u64,
    pub scope_generation: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentScope {
    pub contract: String,
    pub scope_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RenderParams {
    pub contract: String,
    pub producer: ProducerFence,
    pub scope: DocumentScope,
    pub requested_revision: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LeaseFence {
    pub catalog_generation: u64,
    pub package_generation: u64,
    pub service_generation: u64,
    pub artifact_generation: u64,
    pub provider_generation: u64,
    pub runtime_generation: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum LayoutItem {
    #[serde(rename = "tiled")]
    Tiled { terminal_id: String },
    #[serde(rename = "container")]
    Container {
        container_id: String,
        terminal_ids: Vec<String>,
        #[serde(default)]
        selected_terminal_id: Option<String>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalSnapshot {
    pub contract: String,
    pub workspace_id: String,
    pub tab_id: String,
    pub lease_fence: LeaseFence,
    pub fingerprint_sha256: String,
    pub layout: Vec<LayoutItem>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StateSnapshot {
    pub contract: String,
    pub store_id: String,
    pub store_generation: u64,
    pub namespace: String,
    pub revision: u64,
    pub sequence: u64,
    pub values: BTreeMap<String, Value>,
}

#[derive(Serialize)]
struct FingerprintPayload<'a> {
    contract: &'a str,
    workspace_id: &'a str,
    tab_id: &'a str,
    lease_fence: &'a LeaseFence,
    layout: &'a [LayoutItem],
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentDependency {
    pub contract: String,
    pub scope_id: String,
    pub revision: u64,
    pub generation: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Accessibility {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum DocumentNode {
    #[serde(rename = "group")]
    Group {
        id: String,
        accessibility: Accessibility,
        #[serde(default)]
        children: Vec<String>,
    },
    #[serde(rename = "text")]
    Text {
        id: String,
        accessibility: Accessibility,
        text: String,
    },
    #[serde(rename = "terminal_slot")]
    TerminalSlot {
        id: String,
        accessibility: Accessibility,
        terminal_id: String,
    },
}

impl DocumentNode {
    fn id(&self) -> &str {
        match self {
            Self::Group { id, .. } | Self::Text { id, .. } | Self::TerminalSlot { id, .. } => id,
        }
    }

    fn accessibility(&self) -> &Accessibility {
        match self {
            Self::Group { accessibility, .. }
            | Self::Text { accessibility, .. }
            | Self::TerminalSlot { accessibility, .. } => accessibility,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HostDocument {
    pub contract: String,
    pub producer: ProducerFence,
    pub scope: DocumentScope,
    pub revision: u64,
    pub dependencies: Vec<DocumentDependency>,
    pub roots: Vec<String>,
    pub nodes: Vec<DocumentNode>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionError {
    SnapshotInvalid,
    FingerprintInvalid,
    DocumentInvalid,
}

pub fn canonical_fingerprint(snapshot: &TerminalSnapshot) -> Result<String, ProjectionError> {
    Ok(format!(
        "{:x}",
        Sha256::digest(canonical_fingerprint_input(snapshot)?)
    ))
}

pub(crate) fn canonical_fingerprint_input(
    snapshot: &TerminalSnapshot,
) -> Result<Vec<u8>, ProjectionError> {
    serde_json::to_vec(&FingerprintPayload {
        contract: &snapshot.contract,
        workspace_id: &snapshot.workspace_id,
        tab_id: &snapshot.tab_id,
        lease_fence: &snapshot.lease_fence,
        layout: &snapshot.layout,
    })
    .map_err(|_| ProjectionError::SnapshotInvalid)
}

pub fn validate_snapshot(snapshot: &TerminalSnapshot) -> Result<BTreeSet<String>, ProjectionError> {
    if snapshot.contract != "host.terminals.v1"
        || !workspace_id(&snapshot.workspace_id)
        || !tab_id(&snapshot.tab_id)
        || !lower_hex(&snapshot.fingerprint_sha256, 64)
        || snapshot.layout.len() > MAX_LAYOUT_ITEMS
        || fence_values(&snapshot.lease_fence).any(|value| value == 0)
    {
        return Err(ProjectionError::SnapshotInvalid);
    }

    let mut terminals = BTreeSet::new();
    let mut containers = BTreeSet::new();
    for item in &snapshot.layout {
        match item {
            LayoutItem::Tiled { terminal_id: id } => {
                if !valid_terminal_id(id) || !terminals.insert(id.clone()) {
                    return Err(ProjectionError::SnapshotInvalid);
                }
            }
            LayoutItem::Container {
                container_id: id,
                terminal_ids,
                selected_terminal_id,
            } => {
                if !container_id(id)
                    || !containers.insert(id.clone())
                    || terminal_ids.len() > MAX_MEMBERS
                {
                    return Err(ProjectionError::SnapshotInvalid);
                }
                let mut local = BTreeSet::new();
                for terminal in terminal_ids {
                    if !valid_terminal_id(terminal)
                        || !local.insert(terminal.as_str())
                        || !terminals.insert(terminal.clone())
                    {
                        return Err(ProjectionError::SnapshotInvalid);
                    }
                }
                if terminal_ids.is_empty() != selected_terminal_id.is_none()
                    || selected_terminal_id
                        .as_ref()
                        .is_some_and(|selected| !local.contains(selected.as_str()))
                {
                    return Err(ProjectionError::SnapshotInvalid);
                }
            }
        }
    }
    if terminals.len() > MAX_TERMINALS || containers.len() > MAX_CONTAINERS {
        return Err(ProjectionError::SnapshotInvalid);
    }
    if canonical_fingerprint(snapshot)? != snapshot.fingerprint_sha256 {
        return Err(ProjectionError::FingerprintInvalid);
    }
    Ok(terminals)
}

pub fn project(
    params: RenderParams,
    state: &StateSnapshot,
    policies: &BTreeMap<String, PolicyRecord>,
    snapshot: &TerminalSnapshot,
) -> Result<HostDocument, ProjectionError> {
    let terminals = validate_snapshot(snapshot)?;
    let mut dependencies = vec![
        state_snapshot_dependency(state)?,
        DocumentDependency {
            contract: "host.terminals.v1".into(),
            scope_id: format!("{}/{}", snapshot.tab_id, snapshot.fingerprint_sha256),
            revision: 1,
            generation: snapshot.lease_fence.provider_generation,
        },
    ];
    dependencies.sort();
    let mut roots = Vec::with_capacity(snapshot.layout.len());
    let mut nodes = Vec::with_capacity(snapshot.layout.len() + terminals.len() * 2);
    let mut next = 0_usize;
    for item in &snapshot.layout {
        match item {
            LayoutItem::Tiled { terminal_id } => {
                let id = node_id(next);
                next += 1;
                roots.push(id.clone());
                nodes.push(slot(id, terminal_id.clone(), false));
            }
            LayoutItem::Container {
                container_id,
                terminal_ids,
                ..
            } => {
                validate_neutral(terminal_ids).map_err(|_| ProjectionError::DocumentInvalid)?;
                let policy = policies.get(container_id);
                let group_id = node_id(next);
                next += 1;
                roots.push(group_id.clone());
                let mut children = Vec::new();
                let insert_at = nodes.len();
                if let Some(label) = policy.and_then(|record| record.label.as_ref()) {
                    let id = node_id(next);
                    next += 1;
                    children.push(id.clone());
                    nodes.push(DocumentNode::Text {
                        id,
                        accessibility: accessibility("Collection label"),
                        text: label.clone(),
                    });
                }
                for terminal_id in terminal_ids {
                    let is_archived = archived(policy, terminal_id);
                    let id = node_id(next);
                    next += 1;
                    children.push(id.clone());
                    nodes.push(slot(id, terminal_id.clone(), is_archived));
                }
                nodes.insert(
                    insert_at,
                    DocumentNode::Group {
                        id: group_id,
                        accessibility: accessibility("Collection"),
                        children,
                    },
                );
            }
        }
    }
    let document = HostDocument {
        contract: "host.document.v2".into(),
        producer: params.producer,
        scope: params.scope,
        revision: params.requested_revision,
        dependencies,
        roots,
        nodes,
    };
    validate_document(&document, &terminals, true)?;
    Ok(document)
}

pub fn validate_document(
    document: &HostDocument,
    authorized_terminals: &BTreeSet<String>,
    exact_coverage: bool,
) -> Result<(), ProjectionError> {
    if document.contract != "host.document.v2"
        || !valid_producer(&document.producer)
        || !valid_scope(&document.scope)
        || document.revision == 0
        || document.dependencies.len() > 64
        || document.nodes.len() > MAX_NODES
        || document.roots.len() > MAX_NODES
    {
        return Err(ProjectionError::DocumentInvalid);
    }
    if document
        .dependencies
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
        || document.dependencies.iter().any(|dependency| {
            !contract_name(&dependency.contract)
                || !opaque_id(&dependency.scope_id)
                || dependency.revision == 0
                || dependency.generation == 0
        })
    {
        return Err(ProjectionError::DocumentInvalid);
    }

    let mut by_id = BTreeMap::new();
    let mut total_text = 0_usize;
    let mut slots = BTreeSet::new();
    for node in &document.nodes {
        if !opaque_id(node.id()) || by_id.insert(node.id(), node).is_some() {
            return Err(ProjectionError::DocumentInvalid);
        }
        let accessibility = node.accessibility();
        if !bounded_text(&accessibility.name, 1, 256)
            || accessibility
                .description
                .as_ref()
                .is_some_and(|text| !bounded_text(text, 0, 1024))
        {
            return Err(ProjectionError::DocumentInvalid);
        }
        total_text += accessibility.name.chars().count();
        total_text += accessibility
            .description
            .as_ref()
            .map_or(0, |text| text.chars().count());
        match node {
            DocumentNode::Group { children, .. } => {
                if children.len() > MAX_NODES || !all_unique(children) {
                    return Err(ProjectionError::DocumentInvalid);
                }
            }
            DocumentNode::Text { text, .. } => {
                if !bounded_text(text, 0, 4096) {
                    return Err(ProjectionError::DocumentInvalid);
                }
                total_text += text.chars().count();
            }
            DocumentNode::TerminalSlot { terminal_id, .. } => {
                if !valid_terminal_id(terminal_id)
                    || !authorized_terminals.contains(terminal_id)
                    || !slots.insert(terminal_id.as_str())
                {
                    return Err(ProjectionError::DocumentInvalid);
                }
            }
        }
    }
    if total_text > MAX_TOTAL_TEXT_CHARS || !all_unique(&document.roots) {
        return Err(ProjectionError::DocumentInvalid);
    }

    let mut visited = BTreeSet::new();
    let mut owned = BTreeSet::new();
    for root in &document.roots {
        if !owned.insert(root.as_str()) || walk(root, 1, &by_id, &mut visited, &mut owned).is_err()
        {
            return Err(ProjectionError::DocumentInvalid);
        }
    }
    if visited.len() != document.nodes.len()
        || (exact_coverage && slots != authorized_terminals.iter().map(String::as_str).collect())
        || serde_json::to_vec(document).map_or(true, |bytes| bytes.len() > MAX_DOCUMENT_BYTES)
    {
        return Err(ProjectionError::DocumentInvalid);
    }
    Ok(())
}

fn walk<'a>(
    id: &'a str,
    depth: usize,
    nodes: &BTreeMap<&'a str, &'a DocumentNode>,
    visited: &mut BTreeSet<&'a str>,
    owned: &mut BTreeSet<&'a str>,
) -> Result<(), ()> {
    if depth > MAX_DEPTH || !visited.insert(id) {
        return Err(());
    }
    let node = nodes.get(id).ok_or(())?;
    if let DocumentNode::Group { children, .. } = node {
        for child in children {
            if !owned.insert(child) || walk(child, depth + 1, nodes, visited, owned).is_err() {
                return Err(());
            }
        }
    }
    Ok(())
}

fn slot(id: String, terminal_id: String, archived: bool) -> DocumentNode {
    DocumentNode::TerminalSlot {
        id,
        accessibility: accessibility(if archived {
            "Archived terminal"
        } else {
            "Terminal"
        }),
        terminal_id,
    }
}

fn accessibility(name: &str) -> Accessibility {
    Accessibility {
        name: name.into(),
        description: None,
    }
}

pub(crate) fn state_snapshot_dependency(
    state: &StateSnapshot,
) -> Result<DocumentDependency, ProjectionError> {
    let revision = state
        .revision
        .checked_add(1)
        .ok_or(ProjectionError::DocumentInvalid)?;
    let mut digest = Sha256::new();
    digest.update(b"vqro.host.state.document-dependency.v1\0");
    for value in [&state.store_id, &state.namespace] {
        digest.update((value.len() as u64).to_be_bytes());
        digest.update(value.as_bytes());
    }
    Ok(DocumentDependency {
        contract: "host.state.v1".into(),
        scope_id: format!("state_{:x}", digest.finalize()),
        revision,
        generation: state.store_generation,
    })
}

fn node_id(index: usize) -> String {
    format!("node-{index:03}")
}

fn fence_values(fence: &LeaseFence) -> impl Iterator<Item = u64> {
    [
        fence.catalog_generation,
        fence.package_generation,
        fence.service_generation,
        fence.artifact_generation,
        fence.provider_generation,
        fence.runtime_generation,
    ]
    .into_iter()
}

pub fn valid_producer(value: &ProducerFence) -> bool {
    package_id(&value.package_id)
        && opaque_id(&value.service_id)
        && lower_hex(&value.artifact_sha256, 64)
        && value.runtime_generation != 0
        && value.provider_generation != 0
        && value.scope_generation != 0
}

pub fn valid_scope(value: &DocumentScope) -> bool {
    contract_name(&value.contract) && opaque_id(&value.scope_id)
}

fn bounded_text(value: &str, min: usize, max: usize) -> bool {
    let count = value.chars().count();
    (min..=max).contains(&count)
        && !value
            .chars()
            .any(|character| matches!(character as u32, 0x00..=0x1f | 0x7f..=0x9f))
}

fn all_unique(values: &[String]) -> bool {
    let mut found = BTreeSet::new();
    values.iter().all(|value| found.insert(value.as_str()))
}

fn lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_terminal_id(value: &str) -> bool {
    value.len() == 37 && value.starts_with("term_") && lower_hex(&value[5..], 32)
}

fn tab_id(value: &str) -> bool {
    value.len() == 36 && value.starts_with("tab_") && lower_hex(&value[4..], 32)
}

fn container_id(value: &str) -> bool {
    value.len() == 26 && value.starts_with("container_") && lower_hex(&value[10..], 16)
}

fn workspace_id(value: &str) -> bool {
    if !(2..=32).contains(&value.len()) || !value.starts_with('w') {
        return false;
    }
    value[1..]
        .bytes()
        .try_fold(0_u64, |decoded, byte| {
            let digit = b"123456789ABCDEFGHJKMNPQRSTVWXYZ0"
                .iter()
                .position(|candidate| *candidate == byte)? as u64;
            decoded.checked_mul(32)?.checked_add(digit + 1)
        })
        .is_some_and(|decoded| decoded != 0)
}

fn opaque_id(value: &str) -> bool {
    (1..=128).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-".contains(&byte))
}

fn contract_name(value: &str) -> bool {
    value.len() <= 128
        && value.split('.').count() >= 2
        && value.split('.').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

fn package_id(value: &str) -> bool {
    contract_name(value)
}
