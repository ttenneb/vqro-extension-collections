# Structural-shadow budgets

The `0.0.0` package is provisional and unpublished. All limits below are
rejection limits, not truncation targets.

## Runtime contract

- runtime request frame: 256 KiB;
- render parameters: 2 KiB;
- request/call and opaque document identifiers: 128 bytes;
- terminal snapshot response envelope: 64 KiB;
- layout items: 64; terminals: 32; containers: 32; members/container: 64;
- exactly one attempted host call, at depth 1, with one outstanding call;
- document encoding: 128 KiB;
- nodes/roots/group children: 512; dependencies: 64; graph depth: 16;
- text: 4,096 characters/field; accessibility name: 256;
  accessibility description: 1,024; aggregate text: 65,536.

The pure document validator also enforces strict dependency ordering, unique
IDs, complete reachability, acyclicity, single-parent ownership, bounded depth,
unique terminal slots, and exact terminal coverage. Control characters are
rejected. Snapshot validation enforces canonical IDs, nonzero lease fences,
container selection integrity, global identity uniqueness, topology bounds,
and the recomputed canonical fingerprint. Host-call responses require the exact
`type: "host_response"` discriminator. Host-call requests and final document
results crossing WIT use compact JSON with recursively lexicographically sorted
object keys and order-preserving arrays, matching the host component boundary.
The terminal fingerprint input remains compact struct-order JSON by contract.

## Package and component

`xtask verify` enforces the public package-v1 limits before unbounded file
allocation:

- archive and individual file: 8 MiB;
- expanded data: 64 MiB; entries: 10,000;
- manifest: 256 KiB; checksums: 1 MiB;
- path: 512 bytes, 16 components, 128 bytes/component;
- component: 512 KiB.

Archive inventory, order, tar metadata, file modes, paths, and checksums are
exact and deterministic. The archive carries the reviewed WIT, schemas,
provenance, host document fixtures, and terminal fingerprint vector under
`contracts/`. Verification compares hardcoded reviewed SHA-256 values for the
whole package, manifest, component, and every carried contract asset; archive
self-checks alone are insufficient. Component exports are exactly `descriptor`
and `invoke`. Component imports are exactly the public Vqro host/types interfaces
and generated type imports; core lowerings are pinned to `host.call`,
`host.cancelled`, and the component adapter internals. Any WASI import or
unreviewed core/component import fails. `host.log` is not lowered.

The runtime component directly depends only on `serde`, `serde_json`, `sha2`,
and wasm-target-only `wit-bindgen`. The builder uses `anyhow`, `sha2`, `tar`,
`wasmparser`, and `wit-component`. Versions are locked; adding authority or a
dependency requires review. No standalone runtime, host internals, or Vqro
executable integration is present.
