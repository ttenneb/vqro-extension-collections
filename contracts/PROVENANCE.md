# Public contract provenance

These contract files and document fixtures are byte-for-byte snapshots from
`https://github.com/ttenneb/vqro.git` at integrated commit
`36f88d6188c4a1c03fac5f9096595b145a77f308`, fetched independently from branch
`feature/integrated-alpha`. They are inputs to this standalone extension only;
no Vqro implementation source is compiled or imported.

| Local path | Source path | SHA-256 |
| --- | --- | --- |
| `vqro-extension-service/world.wit` | `wit/vqro-extension-service/world.wit` | `625f909dcd26c714e94b0361e4c3fde969c792e0497aaf7477e08f4f73f22daf` |
| `api/vqro-service-v1.schema.json` | `docs/next/api/vqro-service-v1.schema.json` | `3c2712d8b92b4908a40b92360e7fe089e914a88b1cb07640dc8d5c5d7129bdbc` |
| `api/host-terminals-v1.schema.json` | `docs/next/api/host-terminals-v1.schema.json` | `3b0a9fff414d1f2b0ca57b8e26910d4da9ec11a5fe30a7704a4a3acc74680449` |
| `api/host-state-v1.schema.json` | `docs/next/api/host-state-v1.schema.json` at Vqro `976bb81c66354e36625c7b189f7212b6917ea2ec` | `76f18212c69780c5100594598a3aa388ddf70e4d275461ca1cbab9b2418f77bd` |
| `api/host-document-render-v2.schema.json` | `docs/next/api/host-document-render-v2.schema.json` | `a500fef482259ce90772f9e677c3b9a943d006cfd0a75abe7915cf1bc2994776` |
| `api/host-document-v2.schema.json` | `docs/next/api/host-document-v2.schema.json` | `1c433b3b25ca1703204ec51f3f65f02541f2fcd02568105d34da53731d8bef52` |
| `api/host-document-v2.md` | `docs/next/api/host-document-v2.md` at Vqro `976bb81c66354e36625c7b189f7212b6917ea2ec` | `50acbfdb201b1a56006f47a8c6a6cdb20761b0bd1d6d8c7b0dd65a0ddf5d3832` |
| `fixtures/host-document-v2/valid.json` | `docs/next/api/host-document-v2.fixtures/valid.json` | `95018a287650b5de06691b200a091643eb5130b0387918812fddadb07e016163` |
| `fixtures/host-document-v2/invalid-action.json` | `docs/next/api/host-document-v2.fixtures/invalid-action.json` | `10d6a80150e18d0b18d39e6c1f76ea0ed38a4d76f8d961f653a94d18520a637f` |
| `fixtures/host-document-v2/invalid-unknown-field.json` | `docs/next/api/host-document-v2.fixtures/invalid-unknown-field.json` | `2cf6c0d971e06925c553f893a30af0c33049d79b929204a5990199de0a15197f` |
| `fixtures/host-document-v2/invalid-unreachable.json` | `docs/next/api/host-document-v2.fixtures/invalid-unreachable.json` | `020e74c6f94c63f9f6399d2deaf71956877edebb06dd943a18a98c0ba1a8e100` |

`fixtures/host-terminals-v1/fingerprint-vector.json` is a shared compatibility
vector generated independently from the exact private input struct and
serialization order in public host function
`src/extension_host_terminals.rs::snapshot_fingerprint` (source-file SHA-256
`449bb3959c61dd4ffa4cbd90c65df987c8be207e3738e5f8ecb2eec36b3c0d0d`). Its
canonical input hashes to
`ed5988fc573adfda94636fb85ab86064ea55dc802db7abb0185edfe5c86b8fa8`; the
vector-file SHA-256 is
`ace75ef459e96136563ee3cc747d0e2537a8f5cadd0f8af41a95f4ccf3597105`.
Host document validator behavior was independently compared with
`src/host_document_v2.rs` (SHA-256
`011a3c2aba429b31387e1d386db6de2ecc60d9da9fe7b5d92eeb8746ab78747b`).

The additive `host.state.v1` schema and updated `host-document-v2.md` were
independently fetched at the later public composite-dependency commit recorded
in their source-path cells; this does not import host implementation code or
activate the staged gate.

The following are package-owned contracts and fixtures, not Vqro host
snapshots. They pin this component's TPM-neutral planner boundary and do not
claim or grant host execution semantics:

| Local path | Ownership | SHA-256 |
| --- | --- | --- |
| `package/README.md` | `vqro.collections` package | `d24291ef1cbba033393b1dd7a44b48e8be450d3cbba8f640f3f098f58625e7dd` |
| `package/collections-document-action-v1.schema.json` | `vqro.collections` package | `92c8858b40995747a234b8ee17149c000244b82d754265373f1b702a467b89b4` |
| `package/effect-plan-v1.schema.json` | `vqro.collections` package | `929bf84d06d5a707ee5e78dbb1af7b324f0df81088fc9205539a37df9c12bcf1` |
| `fixtures/collections-actions/label-invocation.json` | `vqro.collections` package | `04b55caf32faaad72c06b44f9f0c538a8aff3c36948c9b999c9c38d8dcb70f69` |
| `fixtures/collections-actions/label-effect-plan.json` | `vqro.collections` package | `1a92a639ab3a83fedb07be64622bccd24c0dd5189ec66269c314c78d6253fbae` |

`cargo run --locked -p xtask -- contract-check` verifies every listed local
asset. Packaging carries the same assets and `xtask verify` binds every one to
its hardcoded digest. The final host execution/acceptance gate remains
mandatory; source and fixture compatibility do not activate the provider.
