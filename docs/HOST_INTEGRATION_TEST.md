# Host integration test specification

This is a host-side follow-up specification only. This repository must not
modify or import the Vqro repository. The candidate remains provisional and
unpublished until an authoritative gate passes against Vqro integrated commit
`976bb81c66354e36625c7b189f7212b6917ea2ec` (or an explicitly reviewed
successor preserving the copied public contracts and composite staged gate).

## Fixture

1. Run `cargo run --locked -p xtask -- package` twice from distinct clean
   source paths and `CARGO_TARGET_DIR` values and require identical bytes and
   the hardcoded reviewed package digest.
2. Copy the `.vqrox` into a private host test directory and confirm its package,
   manifest, component, and carried-contract SHA-256 values against the
   hardcoded reviewed values.
3. Do not add it to the production catalog or enable production discovery.

## Required assertions

1. **Package:** validation accepts ID/namespace `vqro.collections`, version
   `0.0.0`, service `collections`, component world
   `vqro:extension/service@1.0.0`, provides exactly `host_document`, and grants
   exactly `host.state.read` and `host.terminals.read`. Ordinary local-file provenance still rejects
   the reserved `vqro.*` claim; test-only curated provenance binds the exact
   repository, commit, package, and manifest digests.
2. **Runtime surface:** descriptor is exactly service `collections`, methods
   `["host.document.render"]`. Instantiation needs no WASI, filesystem,
   network, environment, clock, random, process, or executable lookup.
3. **Read-only calls:** for a valid host-authored render request, record exactly
   one depth-1 `state.snapshot` call with capability `host.state.read` and params
   `{ "contract": "host.state.v1" }`, followed by exactly one depth-1
   `terminals.snapshot` call with capability `host.terminals.read` and params
   `{ "contract": "host.terminals.v1" }`. Both copy identity and namespace.
   Require byte-exact canonical JSON. No retry, write, action, effect, focus, or
   log call is permitted.
4. **Projection:** exercise mixed tiled/container topology, empty containers,
   selection, empty and maximum snapshots. Reconcile only package label/archive
   policy while preserving terminal topology. Require sorted normative state and terminal
   dependencies, top-level producer/scope/revision echo, stable local node IDs,
   preserved ordering, exact terminal coverage, and byte-identical repeated
   output. Assert no pane, geometry, selection, action, or effect field appears.
5. **Rejection:** cover every outer identity/contract/fence mismatch, unknown
   field, malformed response envelope, host error, malformed snapshot,
   legacy `host_call_response` and arbitrary response discriminators,
   fingerprint mismatch, duplicate/missing terminal, invalid selection,
   topology/document/text/encoded bound, graph, and dependency error. Invalid
   requests and pre-call cancellation make zero calls; all post-admission paths
   make at most two, with failures stopping the sequence. Post-call cancellation wins.
   Returned service errors are static and contain no payload text.
6. **Artifact binding and mutation rejection:** mutate the archive, manifest,
   copied contract, or extracted component and require rejection before
   component execution. Revoke or stale any generation/fence and require the
   authoritative host gate to reject acceptance.

The checked-in host golden documents and host-source-confirmed fingerprint
vector provide DTO/validator compatibility evidence only. Passing local source
and fixture checks is not a substitute for this final host execution and
acceptance gate. Passing that gate activates no persistence, endpoint,
render-loop work, user action, effect, mutation, or publication; those remain
out of scope.
