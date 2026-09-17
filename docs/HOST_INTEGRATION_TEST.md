# Host integration test specification

This is a specification only. This repository must not modify or import the
Vqro host repository. The authoritative gate will be implemented as a
coordinated Vqro follow-up after this candidate has a reviewed commit. Until
that gate passes, this branch and every generated artifact are provisional and
non-release.

Test against host commit
`74406386828e182043d48ecf3511d4c4288924ff` or an explicitly reviewed
successor containing the same `vqro.package.v1` and
`vqro:extension/service@1.0.0` contracts.

## Fixture setup

1. Build `dist/vqro-collections-candidate.vqrox` in this repository with
   `cargo run --locked -p xtask -- package`.
2. Copy the artifact into a private host test directory.
3. Compute its package and embedded manifest SHA-256 values at test runtime.
4. Do not add it to the production catalog.

## Required assertions

1. **Package validation:** `validate_package_file(path, Some(package_sha256))`
   succeeds. Assert package ID `vqro.collections`, version `0.0.0`, minimum Vqro version `0.9.0`, namespace
   `vqro.collections`, one component service named `collections`, the exact
   world, and empty `provides` and `capabilities`.
2. **Ordinary source rejection:** attempting `install_local_file` with ordinary
   local-file provenance rejects the `vqro.*` namespace claim. Assert the
   typed/stable rejection category where available, not only message text.
3. **Synthetic curated acceptance:** construct test-only curated provenance
   with publisher `vqro`, the exact repository/commit, exact package digest,
   exact manifest digest, and authorized namespace `vqro.collections`.
   Registration and installation succeed. This provenance must remain local to
   the test and must not alter `extensions/index-v1.json`.
4. **Runtime instantiation:** construct the installed component runtime from the
   verified artifact and start a nonzero generation. No executable lookup,
   process spawn, environment, filesystem, network, clock, random, or WASI
   linkage is required.
5. **Empty descriptor:** startup returns service ID `collections` and an empty
   method set. Do not invoke a synthetic feature or conformance method.
6. **No host calls:** use a counting/rejecting `RuntimeHostCallHandler`; assert
   its count remains zero across instantiate, descriptor negotiation, and
   orderly stop. If `invoke` is tested directly, it must return the component's
   `no_authority` service error and still make zero host calls.
7. **Artifact binding:** mutate either the archive or extracted component and
   assert revalidation/runtime construction fails before component execution.

The test passes only as package/runtime plumbing. It grants no Collections
feature authority and satisfies none of the M3 mutation, migration, document,
or cutover deletion gates.
