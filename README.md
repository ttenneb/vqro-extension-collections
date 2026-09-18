# Vqro Collections R0 package

This repository publishes the deterministic, exact-Git opt-in `vqro.collections` component
package. Its service ID is `collections`; its exact advertised methods are:

- `vqro.collections.document-profile.render.v1`
- `vqro.collections.actions.list.v1`
- `vqro.collections.action.plan.v1`

The package declares only `host.state.read` and `host.terminals.read`. It receives no direct state
write, terminal mutation, filesystem, network, process, clock, random, environment, socket, or
ambient WASI capability.

## Profile and actions

Profile and action-list requests use the host-authored `host.document.render.v2` request. The
component performs one depth-1 `state.snapshot` and one depth-1 `terminals.snapshot`, validates the
full response envelopes and snapshots, and produces either:

- `vqro.collections.document-profile.v1`, containing an unchanged action-free
  `host.document.v2` plus the exact optional Collection labels; or
- `vqro.collections.actions.v1`, declaring exactly one `set_label` action per Collection and one
  `set_archived` action per member terminal.

Tiled terminals are omitted. Each neutral container maps to
`collection:<container_id>`. Active terminals are direct children in neutral order. Archived
terminals are under the final `archive:<container_id>` subgroup in neutral order. Empty containers
remain representable for an unpublished labeled-create reservation.

An action-plan request uses `vqro.collections.action-invocation.v1`. The component performs one
state snapshot, checks the invocation's exact state CAS versions, validates its action-specific
dependency, updates one strict package policy record, and returns
`vqro.collections.effect-plan.v1`. The plan echoes the opaque ticket and idempotency key and proposes
exactly one set-or-delete for that container key. The component never executes the plan. Ticket
validity, current topology dependencies, persistence, and publication remain host authority.

The frozen schemas and fixtures under [`contracts/`](contracts/) are byte-identical to the host R0
contract baseline and are carried inside the `.vqrox` checksum inventory. Package policy source
profiles remain outside the artifact.

## Distribution and authority

`dist/vqro-collections.vqrox` is a reviewed deterministic artifact committed in the same exact Git
revision as its source. Vqro installation fetches that exact revision and archive path, verifies the
catalog-bound package, manifest, and component digests, and runs no repository build command.
Maintainers use `xtask` to reproduce and verify the committed artifact.

The package and curated catalog grant no provider or feature authority by themselves. Vqro must
still grant bounded read leases, audit the result, and perform the accepted journaled authority
cutover. Generic host terminal access remains independent.

## Build and verify

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo run --locked -p xtask -- contract-check
cargo run --locked -p xtask -- package dist/vqro-collections.vqrox
cargo run --locked -p xtask -- verify dist/vqro-collections.vqrox
```

Run `package` twice from clean, distinct target directories and compare bytes before updating the
reviewed digest constants. See [`docs/BUDGETS.md`](docs/BUDGETS.md) and
[`docs/HOST_INTEGRATION_TEST.md`](docs/HOST_INTEGRATION_TEST.md).
