//! Frozen R0 Collections document-profile and typed action declarations.

use crate::policy::{archived, validate_neutral, PolicyRecord};
use crate::projection::{
    state_snapshot_dependency, validate_document, validate_snapshot, Accessibility,
    DocumentDependency, DocumentNode, HostDocument, LayoutItem, ProjectionError, RenderParams,
    StateSnapshot, TerminalSnapshot,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

pub const PROFILE_CONTRACT: &str = "vqro.collections.document-profile.v1";
pub const ACTIONS_CONTRACT: &str = "vqro.collections.actions.v1";
pub const PROFILE_METHOD: &str = "vqro.collections.document-profile.render.v1";
pub const ACTIONS_METHOD: &str = "vqro.collections.actions.list.v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentProfile {
    pub contract: &'static str,
    pub document: HostDocument,
    pub collections: Vec<DocumentCollection>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentCollection {
    pub container_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActionDeclarations {
    pub contract: &'static str,
    pub document_revision: u64,
    pub revision: u64,
    pub actions: Vec<ActionDeclaration>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActionDeclaration {
    pub id: String,
    pub subject_node_id: String,
    pub label: &'static str,
    pub action: ActionKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    SetLabel,
    SetArchived,
}

pub fn project_profile(
    params: RenderParams,
    state: &StateSnapshot,
    policies: &BTreeMap<String, PolicyRecord>,
    snapshot: &TerminalSnapshot,
) -> Result<DocumentProfile, ProjectionError> {
    validate_snapshot(snapshot)?;
    if params.producer.package_id != "vqro.collections"
        || params.producer.service_id != "collections"
        || params.scope.contract != "host.tab.v1"
        || params.scope.scope_id != snapshot.tab_id
    {
        return Err(ProjectionError::DocumentInvalid);
    }
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

    let mut roots = Vec::new();
    let mut nodes = Vec::new();
    let mut collections = Vec::new();
    let mut authorized_terminals = BTreeSet::new();
    for item in &snapshot.layout {
        let LayoutItem::Container {
            container_id,
            terminal_ids,
            ..
        } = item
        else {
            continue;
        };
        validate_neutral(terminal_ids).map_err(|_| ProjectionError::DocumentInvalid)?;
        let policy = policies.get(container_id);
        let label = policy.and_then(|record| record.label.clone());
        let effective_label = label
            .as_deref()
            .filter(|label| !label.is_empty())
            .unwrap_or("Collection")
            .to_string();
        if !valid_profile_text(&effective_label, false) {
            return Err(ProjectionError::DocumentInvalid);
        }

        let root_id = format!("collection:{container_id}");
        roots.push(root_id.clone());
        collections.push(DocumentCollection {
            container_id: container_id.clone(),
            label,
        });

        let mut active = Vec::new();
        let mut archived_ids = Vec::new();
        for terminal_id in terminal_ids {
            authorized_terminals.insert(terminal_id.clone());
            if archived(policy, terminal_id) {
                archived_ids.push(terminal_id);
            } else {
                active.push(terminal_id);
            }
        }
        let mut children = active
            .iter()
            .map(|terminal_id| format!("terminal:{terminal_id}"))
            .collect::<Vec<_>>();
        if !archived_ids.is_empty() {
            children.push(format!("archive:{container_id}"));
        }
        nodes.push(DocumentNode::Group {
            id: root_id,
            accessibility: accessibility(&effective_label),
            children,
        });
        for terminal_id in active {
            nodes.push(slot(terminal_id, "Terminal"));
        }
        if !archived_ids.is_empty() {
            nodes.push(DocumentNode::Group {
                id: format!("archive:{container_id}"),
                accessibility: accessibility("Archive"),
                children: archived_ids
                    .iter()
                    .map(|terminal_id| format!("terminal:{terminal_id}"))
                    .collect(),
            });
            for terminal_id in archived_ids {
                nodes.push(slot(terminal_id, "Archived terminal"));
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
    validate_document(&document, &authorized_terminals, true)?;
    Ok(DocumentProfile {
        contract: PROFILE_CONTRACT,
        document,
        collections,
    })
}

pub fn declare_actions(profile: &DocumentProfile) -> ActionDeclarations {
    let archived = profile
        .document
        .nodes
        .iter()
        .filter_map(|node| match node {
            DocumentNode::Group { id, children, .. } if id.starts_with("archive:") => {
                Some(children.iter().map(String::as_str))
            }
            _ => None,
        })
        .flatten()
        .collect::<BTreeSet<_>>();
    let mut actions = Vec::new();
    for collection in &profile.collections {
        let subject_node_id = format!("collection:{}", collection.container_id);
        actions.push(ActionDeclaration {
            id: format!("set-label:{}", collection.container_id),
            subject_node_id,
            label: "Rename",
            action: ActionKind::SetLabel,
        });
    }
    for node in &profile.document.nodes {
        let DocumentNode::TerminalSlot {
            id, terminal_id, ..
        } = node
        else {
            continue;
        };
        actions.push(ActionDeclaration {
            id: format!("set-archived:{terminal_id}"),
            subject_node_id: id.clone(),
            label: if archived.contains(id.as_str()) {
                "Restore"
            } else {
                "Archive"
            },
            action: ActionKind::SetArchived,
        });
    }
    ActionDeclarations {
        contract: ACTIONS_CONTRACT,
        document_revision: profile.document.revision,
        revision: profile.document.revision,
        actions,
    }
}

fn slot(terminal_id: &str, name: &str) -> DocumentNode {
    DocumentNode::TerminalSlot {
        id: format!("terminal:{terminal_id}"),
        accessibility: accessibility(name),
        terminal_id: terminal_id.into(),
    }
}

fn accessibility(name: &str) -> Accessibility {
    Accessibility {
        name: name.into(),
        description: None,
    }
}

fn valid_profile_text(value: &str, allow_empty: bool) -> bool {
    (allow_empty || !value.is_empty())
        && value.len() <= 4096
        && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projection::{DocumentScope, LeaseFence, ProducerFence};
    use serde_json::json;

    const TERM1: &str = "term_11111111111111111111111111111111";
    const TERM2: &str = "term_22222222222222222222222222222222";
    const CONTAINER: &str = "container_000000000000002a";

    fn fixture() -> (
        RenderParams,
        StateSnapshot,
        BTreeMap<String, PolicyRecord>,
        TerminalSnapshot,
    ) {
        let mut snapshot = TerminalSnapshot {
            contract: "host.terminals.v1".into(),
            workspace_id: "w1".into(),
            tab_id: "tab_11111111111111111111111111111111".into(),
            lease_fence: LeaseFence {
                catalog_generation: 1,
                package_generation: 2,
                service_generation: 3,
                artifact_generation: 4,
                provider_generation: 5,
                runtime_generation: 6,
            },
            fingerprint_sha256: String::new(),
            layout: vec![LayoutItem::Container {
                container_id: CONTAINER.into(),
                terminal_ids: vec![TERM1.into(), TERM2.into()],
                selected_terminal_id: Some(TERM1.into()),
            }],
        };
        snapshot.fingerprint_sha256 = crate::projection::canonical_fingerprint(&snapshot).unwrap();
        let state = StateSnapshot {
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
        };
        let policies = crate::policy::decode_namespace(&state.values).unwrap();
        let params = RenderParams {
            contract: "host.document.render.v2".into(),
            producer: ProducerFence {
                package_id: "vqro.collections".into(),
                service_id: "collections".into(),
                artifact_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .into(),
                runtime_generation: 6,
                provider_generation: 5,
                scope_generation: 7,
            },
            scope: DocumentScope {
                contract: "host.tab.v1".into(),
                scope_id: snapshot.tab_id.clone(),
            },
            requested_revision: 7,
        };
        (params, state, policies, snapshot)
    }

    #[test]
    fn profile_has_canonical_collection_archive_and_terminal_nodes() {
        let (params, state, policies, snapshot) = fixture();
        let profile = project_profile(params, &state, &policies, &snapshot).unwrap();
        assert_eq!(profile.collections[0].label.as_deref(), Some("Helpers"));
        assert_eq!(profile.document.roots, [format!("collection:{CONTAINER}")]);
        assert!(profile.document.nodes.iter().any(|node| matches!(node,
            DocumentNode::Group { id, .. } if id == &format!("archive:{CONTAINER}"))));
        let declarations = declare_actions(&profile);
        assert_eq!(declarations.actions.len(), 3);
        assert_eq!(declarations.actions[2].label, "Restore");
    }

    #[test]
    fn tiled_terminals_are_not_collections_profile_roots() {
        let (params, state, policies, mut snapshot) = fixture();
        snapshot.layout.insert(
            0,
            LayoutItem::Tiled {
                terminal_id: "term_33333333333333333333333333333333".into(),
            },
        );
        snapshot.fingerprint_sha256 = crate::projection::canonical_fingerprint(&snapshot).unwrap();
        let profile = project_profile(params, &state, &policies, &snapshot).unwrap();
        assert_eq!(profile.document.roots.len(), 1);
        assert_eq!(
            profile
                .document
                .nodes
                .iter()
                .filter(|node| matches!(node, DocumentNode::TerminalSlot { .. }))
                .count(),
            2
        );
    }
}
