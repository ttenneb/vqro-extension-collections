# Vqro Collections semantic shadow

This repository builds a **provisional, unpublished `0.0.0`** external Vqro
component. Its single service is `collections`; its exact method set is
`["host.document.render"]`. The service provides `host_document` and has one
capability grants, exactly `host.state.read` and `host.terminals.read`.

The method strictly accepts a host-authored `host.document.render.v2` request,
performs exactly one depth-1 `state.snapshot` followed by exactly one depth-1
`terminals.snapshot` call through the public `vqro:extension/host@1.0.0`
interface, validates both complete envelopes and snapshots, and returns a
deterministic `host.document.v2` semantic shadow with two sorted dependencies.
The host response decoder requires the exact `RuntimeHostCallResponseV1`
discriminator `type: "host_response"`; legacy or arbitrary spellings are
rejected. Both outbound WIT-boundary payloads—the `RuntimeHostCallV1` request
and final document—use compact JSON with object keys recursively sorted
lexicographically; array order
is preserved. The terminal fingerprint input deliberately remains compact JSON
in its normative struct field order and is not passed through that serializer.
Tiled terminals become terminal-slot roots. Containers become ordered group
roots, including empty containers. A strict package-owned policy record may add
an optional text label and mark current members with the static accessibility
name `Archived terminal`; topology, identity, membership, and order remain
exclusively terminal-snapshot-owned. Stale archive IDs are not rendered.
Profiles under [`profiles/`](profiles/) define this package policy but remain
source-only and are deliberately excluded from `.vqrox` artifacts.

This extension has no mutation, action/effect, focus, filesystem, network,
WASI, process, or self-spawn authority. It does not persist or cache documents.
Cancellation is checked around each host call. Errors are static and never include request, snapshot,
or host-error content.

The exact public contract snapshots and authoritative host document fixtures
used by the implementation are under [`contracts/`](contracts/). They are also
carried inside the `.vqrox` at the same paths and covered by its canonical
checksum inventory. [`contracts/PROVENANCE.md`](contracts/PROVENANCE.md) records
the independently fetched source paths, reviewed Vqro commits, and SHA-256
values. The additive `host.state.v1` schema is pinned from `976bb81c`. It also records
the host-source-confirmed terminal fingerprint vector. `xtask` binds the final
package, manifest, component, and every carried contract asset to hardcoded
reviewed digests in addition to structural import/export and archive checks.

## Build and verify

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo run --locked -p xtask -- contract-check
cargo run --locked -p xtask -- package
cargo run --locked -p xtask -- verify dist/vqro-collections-candidate.vqrox
```

Run `package` twice from clean, distinct source and target directories and
compare the output bytes for reproducibility. Generated `dist/` artifacts are
ignored and must not be committed or published. Linux, macOS, and Windows CI
each perform two such clean builds; no CI job publishes them. Public-source and
fixture compatibility is not host execution or acceptance: the final
coordinated host execution gate remains mandatory.
See [`docs/BUDGETS.md`](docs/BUDGETS.md) and
[`docs/HOST_INTEGRATION_TEST.md`](docs/HOST_INTEGRATION_TEST.md).
