# Vqro Collections component candidate

This branch contains a **provisional, non-release, zero-authority candidate**
for a future Vqro Collections package. It currently provides no user-visible
Collections behavior, advertises no service methods or capabilities, and makes
no host calls.

Its bounded purpose is to prove that the repository can independently build a
WASI-free component and a deterministic `vqro.package.v1` archive without
importing Vqro's private Rust code or spawning the Vqro executable.

Real Collection state, documents, and actions remain blocked on public M2 host
contracts. The package ID and namespace also require synthetic curated
provenance during host integration; an ordinary local-file install must reject
the `vqro.*` claim. The planned first-support floor is Vqro 0.9.0, but this
artifact must not be published before that release or before the coordinated
authoritative Vqro host integration gate passes.

## Build and verify

```sh
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo run --locked -p xtask -- contract-check
cargo run --locked -p xtask -- package
cargo run --locked -p xtask -- verify dist/vqro-collections-candidate.vqrox
```

Generated `dist/` artifacts are intentionally ignored and must not be
committed. Canonical reproducibility output is non-release Linux evidence only;
macOS and Windows jobs prove build/package portability but do not publish or
define a canonical digest. See `docs/HOST_INTEGRATION_TEST.md` for the
coordinated authoritative host-side follow-up gate and `docs/BUDGETS.md` for
candidate budgets.
