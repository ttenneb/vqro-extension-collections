# Host integration test specification

This is a host-side follow-up specification only. This repository must not
modify or import the Vqro repository. The candidate remains provisional and
unpublished until an authoritative gate passes against Vqro integrated commit
`36f88d6188c4a1c03fac5f9096595b145a77f308` (or an explicitly reviewed
successor preserving the copied public contracts).

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
   exactly `host.terminals.read`. Ordinary local-file provenance still rejects
   the reserved `vqro.*` claim; test-only curated provenance binds the exact
   repository, commit, package, and manifest digests.
2. **Runtime surface:** descriptor is exactly service `collections`, methods
   `["host.document.render"]`. Instantiation needs no WASI, filesystem,
   network, environment, clock, random, process, or executable lookup.
3. **Read-only call:** for a valid host-authored render request, record exactly
   one depth-1 call with copied outer identity, namespace `vqro.collections`,
   capability `host.terminals.read`, method `terminals.snapshot`, and params
   `{ "contract": "host.terminals.v1" }`. Require byte-exact compact JSON with
   recursively lexicographically sorted object keys and preserved array order
   for this call and the final document returned across WIT. The terminal
   fingerprint vector remains compact struct-order JSON. No state call,
   mutation, action, effect, focus, log, or second call is permitted.
4. **Projection:** exercise mixed tiled/container topology, empty containers,
   selection, empty and maximum snapshots. Require one normative terminal
   dependency, top-level producer/scope/revision echo, stable local node IDs,
   preserved ordering, exact terminal coverage, and byte-identical repeated
   output. Assert no label, archive, pane, geometry, selection, action, or
   effect field appears.
5. **Rejection:** cover every outer identity/contract/fence mismatch, unknown
   field, malformed response envelope, host error, malformed snapshot,
   legacy `host_call_response` and arbitrary response discriminators,
   fingerprint mismatch, duplicate/missing terminal, invalid selection,
   topology/document/text/encoded bound, graph, and dependency error. Invalid
   requests and pre-call cancellation make zero calls; all post-admission paths
   make at most one. Post-call cancellation wins over success or host error.
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
