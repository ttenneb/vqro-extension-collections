# Candidate budgets

This branch is provisional and non-release. Budgets constrain package plumbing;
they are not performance claims for a Collections product implementation.

## Enforced deterministic limits

`xtask verify` fails before unbounded file allocation when any package exceeds
the corresponding public host package-v1 limit:

- archive: 8 MiB;
- individual file: 8 MiB;
- expanded data: 64 MiB;
- entries: 10,000;
- manifest: 256 KiB;
- checksum inventory: 1 MiB;
- path: 512 bytes, 16 components, 128 bytes per component.

The candidate additionally limits `services/collections.wasm` to 512 KiB. It
rejects unsafe, non-portable, case-colliding, file/descendant-colliding, linked,
special, unsorted, or noncanonical tar entries. Component imports and exports
are exact-budget checks, and any core or WASI import fails verification.

## Dependency budget

The runtime component has one direct build dependency (`wit-bindgen`). The
builder has five direct dependencies (`anyhow`, `sha2`, `tar`, `wasmparser`, and
`wit-component`). Versions are locked; adding a direct dependency or exceeding
60 packages in `cargo metadata --locked` requires explicit review. Standalone
Wasmtime is intentionally absent because runtime behavior belongs to the
coordinated authoritative Vqro host gate.

## Observational build budget

These are review thresholds, not portable hard failures. Linux CI records
`/usr/bin/time -v`, target-directory storage, the dependency tree, component
size, and package size. A result above any threshold requires investigation:

- clean package command wall time: 120 seconds;
- peak resident set size: 768 MiB;
- clean Cargo target storage: 600 MiB;
- component: 512 KiB (also enforced);
- package: 8 MiB (also enforced).

Baseline observation on Linux x86-64 with Rust 1.96.1 and an empty external
`CARGO_TARGET_DIR`:

- elapsed: 17.53 seconds;
- peak RSS: 597,616 KiB;
- target storage: 488,712 KiB;
- locked packages: 55;
- component: 19,517 bytes;
- package: approximately 38 KiB.

macOS and Windows CI run workspace, component, and package checks for
portability. Only the distinct-path Linux job records canonical reproducibility
evidence, and that evidence remains non-release.
