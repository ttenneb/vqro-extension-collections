# Vqro Collections structural shadow

This repository builds a **provisional, unpublished `0.0.0`** external Vqro
component. Its single service is `collections`; its exact method set is
`["host.document.render"]`. The service provides `host_document` and has one
capability grant, `host.terminals.read`.

The method strictly accepts a host-authored `host.document.render.v2` request,
performs exactly one depth-1 `terminals.snapshot` call through the public
`vqro:extension/host@1.0.0` interface, validates the complete accepted envelope
and `host.terminals.v1` snapshot (including the recomputed canonical SHA-256
fingerprint), and returns a deterministic `host.document.v2` structural shadow.
The host response decoder requires the exact `RuntimeHostCallResponseV1`
discriminator `type: "host_response"`; legacy or arbitrary spellings are
rejected. Both outbound WIT-boundary payloads—the `RuntimeHostCallV1` request
and final document—use compact JSON with object keys recursively sorted
lexicographically; array order
is preserved. The terminal fingerprint input deliberately remains compact JSON
in its normative struct field order and is not passed through that serializer.
Tiled terminals become terminal-slot roots. Containers become ordered group
roots, including empty containers. Accessibility strings are local static text;
no host labels, archive data, pane IDs, geometry, selection, actions, or effects
are copied.

This extension owns no collection state and has no mutation, action/effect,
focus, filesystem, network, WASI, process, or self-spawn authority. It does not
persist or cache documents. Cancellation is checked immediately before and
after the sole host call. Errors are static and never include request, snapshot,
or host-error content.

The exact public contract snapshots and authoritative host document fixtures
used by the implementation are under [`contracts/`](contracts/). They are also
carried inside the `.vqrox` at the same paths and covered by its canonical
checksum inventory. [`contracts/PROVENANCE.md`](contracts/PROVENANCE.md) records
the independently fetched source paths, Vqro integrated commit
`36f88d6188c4a1c03fac5f9096595b145a77f308`, and SHA-256 values. It also records
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
