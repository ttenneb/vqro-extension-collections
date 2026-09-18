# Collections policy source profile

Status: **package-owned-source-profile-not-host-contract**.

This directory defines a source-only compatibility profile owned by the
`vqro.collections` package. It describes only package interpretation of
label/archive policy values in the package namespace. It is not a Vqro host
contract and grants no state, terminal, document, migration, persistence,
activation, or effect-execution authority.

The profiles are deliberately excluded from the `.vqrox`. The current component
uses the equivalent package-owned production decoder for rendering and pure
action planning; these files remain source-only test and documentation material,
not packaged runtime input. The migration adapter and pinned legacy shape are
documented in [`collections-migration-v1.md`](collections-migration-v1.md).
See [`collections-policy-v1.md`](collections-policy-v1.md)
for the normative package-owned rules and [`collections-policy-v1.schema.json`](collections-policy-v1.schema.json)
for the structural value schema.

Fixtures under [`fixtures/`](fixtures/) are package test vectors, not host
fixtures. Host-side mirror planning or similarly shaped private implementation
is not authority for this profile and is not imported by this repository.
