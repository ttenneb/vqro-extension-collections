# Host integration gate

This package is independently buildable. Production admission still requires a host-side test at an
explicit Vqro revision preserving the copied R0 contracts.

## Artifact fixture

1. Build `dist/vqro-collections.vqrox` twice from clean source and target directories.
2. Require identical bytes and the hardcoded reviewed package digest.
3. Verify the package, manifest, component, and every carried contract digest.
4. Bind curated test provenance to the exact repository, 40-character commit, archive path, and
   three artifact digests. Curated status alone grants no runtime or feature authority.

## Required assertions

1. **Package:** ID/namespace `vqro.collections`, version `1.0.0`, service `collections`, world
   `vqro:extension/service@1.0.0`, provides `host_document`, and capabilities exactly
   `host.state.read` plus `host.terminals.read`.
2. **Runtime:** descriptor service `collections`; methods exactly
   `vqro.collections.document-profile.render.v1`, `vqro.collections.actions.list.v1`, and
   `vqro.collections.action.plan.v1`; no ambient WASI imports.
3. **Profile reads:** profile and action-list methods each make one depth-1 `state.snapshot` followed
   by one depth-1 `terminals.snapshot`, with exact capability/method/namespace/identity envelopes.
   There is no retry and no render-loop call.
4. **Profile:** require one root per neutral container, no tiled roots, canonical Collection/archive/
   terminal node IDs, exact terminal coverage, ordered active/archive partitions, exact optional
   label sidecar, sorted state/terminal dependencies, and deterministic bytes.
5. **Declarations:** require one `set_label` per root and one `set_archived` per terminal slot, with
   exact typed subjects and profile revision binding.
6. **Action plan:** make one state snapshot call; require exact expected state versions, typed
   action-specific dependency, opaque ticket/idempotency echo, and one container-key set-or-delete.
   The component receives no write capability and never executes the plan.
7. **Rejection:** unknown fields, malformed identities, response envelopes, snapshots, fingerprints,
   policy, dependencies, controls, bounds, stale state, cross-subject action IDs, cancellation, and
   replaced generations fail closed with static errors and bounded call counts.
8. **Host audit:** validate the profile and declarations with the host R0 validators; compare both
   generation-1 compatibility projections to the frozen fixtures; reject stale/canceled completion.
9. **Artifact binding:** any archive, manifest, carried contract, or component mutation fails before
   component execution.

Passing this gate does not perform the authority cutover. R1/R2 still supply bounded workers, read
leases, nonpublishing admission, and rebuildable shadow evidence before R3 can journal and publish
package authority.
