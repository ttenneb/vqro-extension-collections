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
| `api/host-document-render-v2.schema.json` | `docs/next/api/host-document-render-v2.schema.json` | `a500fef482259ce90772f9e677c3b9a943d006cfd0a75abe7915cf1bc2994776` |
| `api/host-document-v2.schema.json` | `docs/next/api/host-document-v2.schema.json` | `1c433b3b25ca1703204ec51f3f65f02541f2fcd02568105d34da53731d8bef52` |
| `api/host-document-v2.md` | `docs/next/api/host-document-v2.md` | `cd7079f829f9af4b5e4e943c3f34ad77911f8dfc43fb65855e5a07c8c5aa55b1` |
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

`cargo run --locked -p xtask -- contract-check` verifies every listed local
asset. Packaging carries the same assets and `xtask verify` binds every one to
its hardcoded digest. The final host execution/acceptance gate remains
mandatory; source and fixture compatibility do not activate the provider.
