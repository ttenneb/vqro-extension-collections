# Public contract provenance

This repository compiles no Vqro implementation source. The WIT, generic host schemas, and host-document fixtures below are reviewed snapshots from `https://github.com/ttenneb/vqro.git`; their historical source revisions remain documented by Git history. The Collections R0 assets are byte-identical snapshots from Vqro commit `d0f336cb` on `plan/extensions-mvp-adversarial-review`.

## Generic host contracts

| Local path | Source path | SHA-256 |
| --- | --- | --- |
| `vqro-extension-service/world.wit` | `wit/vqro-extension-service/world.wit` | `625f909dcd26c714e94b0361e4c3fde969c792e0497aaf7477e08f4f73f22daf` |
| `api/vqro-service-v1.schema.json` | `docs/next/api/vqro-service-v1.schema.json` | `3c2712d8b92b4908a40b92360e7fe089e914a88b1cb07640dc8d5c5d7129bdbc` |
| `api/host-state-v1.schema.json` | `docs/next/api/host-state-v1.schema.json` | `76f18212c69780c5100594598a3aa388ddf70e4d275461ca1cbab9b2418f77bd` |
| `api/host-terminals-v1.schema.json` | `docs/next/api/host-terminals-v1.schema.json` | `3b0a9fff414d1f2b0ca57b8e26910d4da9ec11a5fe30a7704a4a3acc74680449` |
| `api/host-document-render-v2.schema.json` | `docs/next/api/host-document-render-v2.schema.json` | `a500fef482259ce90772f9e677c3b9a943d006cfd0a75abe7915cf1bc2994776` |
| `api/host-document-v2.schema.json` | `docs/next/api/host-document-v2.schema.json` | `1c433b3b25ca1703204ec51f3f65f02541f2fcd02568105d34da53731d8bef52` |
| `api/host-document-v2.md` | `docs/next/api/host-document-v2.md` | `50acbfdb201b1a56006f47a8c6a6cdb20761b0bd1d6d8c7b0dd65a0ddf5d3832` |
| `fixtures/host-document-v2/valid.json` | `docs/next/api/host-document-v2.fixtures/valid.json` | `95018a287650b5de06691b200a091643eb5130b0387918812fddadb07e016163` |
| `fixtures/host-document-v2/invalid-action.json` | `docs/next/api/host-document-v2.fixtures/invalid-action.json` | `10d6a80150e18d0b18d39e6c1f76ea0ed38a4d76f8d961f653a94d18520a637f` |
| `fixtures/host-document-v2/invalid-unknown-field.json` | `docs/next/api/host-document-v2.fixtures/invalid-unknown-field.json` | `2cf6c0d971e06925c553f893a30af0c33049d79b929204a5990199de0a15197f` |
| `fixtures/host-document-v2/invalid-unreachable.json` | `docs/next/api/host-document-v2.fixtures/invalid-unreachable.json` | `020e74c6f94c63f9f6399d2deaf71956877edebb06dd943a18a98c0ba1a8e100` |

The terminal fingerprint vector is independently generated from the public host canonical input shape.

## Frozen Collections R0 contracts and fixtures

| Local path | Vqro source path | SHA-256 |
| --- | --- | --- |
| `package/README.md` | `docs/next/api/vqro-collections-v1.md` | `ff3dc63baf00108dc465b30b5adc98d5317de58e494bb74c1cf20f67f741fef2` |
| `package/vqro-collections-action-invocation-v1.schema.json` | `docs/next/api/vqro-collections-action-invocation-v1.schema.json` | `59ee8f1d359954428038c16f75b8999af7698c6185ed9b71dda806ea20d206e9` |
| `package/vqro-collections-actions-v1.schema.json` | `docs/next/api/vqro-collections-actions-v1.schema.json` | `3078eed5a4b876ad9bb7b462d290924b0b6527ec6be8e7f2c279286e66856f0b` |
| `package/vqro-collections-document-profile-v1.schema.json` | `docs/next/api/vqro-collections-document-profile-v1.schema.json` | `aa77062beecc223ebc6068ede0887406e584dbd1507cb9241eff0e0dd8e30b2a` |
| `package/vqro-collections-effect-plan-v1.schema.json` | `docs/next/api/vqro-collections-effect-plan-v1.schema.json` | `5fadc52b3fff7df6532f6e9603defc27681c57cfe338bc442b5a7de7dbc5bc4f` |
| `package/vqro-collections-production-pin-v1.schema.json` | `docs/next/api/vqro-collections-production-pin-v1.schema.json` | `dc4bf74ee492067644db6f24bd39a878efc05afefa6dad39eaf84c9d32ea4923` |
| `fixtures/vqro-collections-v1/actions-valid.json` | `docs/next/api/vqro-collections-v1.fixtures/actions-valid.json` | `d2c552b9d1afe194b3175e532d85e3747e5a6d1456d8e207a398e8f06b1a4f83` |
| `fixtures/vqro-collections-v1/cancellation-before-commit.json` | `docs/next/api/vqro-collections-v1.fixtures/cancellation-before-commit.json` | `7a8ea24cc353e3948926e930b95e15652eb5be43c6d85e54bc172e8839fb4c98` |
| `fixtures/vqro-collections-v1/document-profile-invalid-root-label.json` | `docs/next/api/vqro-collections-v1.fixtures/document-profile-invalid-root-label.json` | `700c92514235523dd369dbea694224a985cc1e70ce57fc7207507726f1f0cf79` |
| `fixtures/vqro-collections-v1/document-profile-valid.json` | `docs/next/api/vqro-collections-v1.fixtures/document-profile-valid.json` | `f7aa6ee90235f2dfca26c51cfdbf8cbf0983a8150e11a844494ab347c7b7e182` |
| `fixtures/vqro-collections-v1/effect-plan-invalid-ticket.json` | `docs/next/api/vqro-collections-v1.fixtures/effect-plan-invalid-ticket.json` | `8dcc305725af91e0c11c34ec46ab75856c4c8f4ace0106cebcfb4525300e229b` |
| `fixtures/vqro-collections-v1/effect-plan-valid.json` | `docs/next/api/vqro-collections-v1.fixtures/effect-plan-valid.json` | `206b41ede9eea60a90bd95e6d1be005d2659a291ae265de898c48db92e5e8fd0` |
| `fixtures/vqro-collections-v1/invocation-set-archived-valid.json` | `docs/next/api/vqro-collections-v1.fixtures/invocation-set-archived-valid.json` | `482dabd84f8d7fae2cd467b4c611fc009ae365312527a091dfcfaf41eb2fb0dc` |
| `fixtures/vqro-collections-v1/invocation-set-label-invalid-dependency.json` | `docs/next/api/vqro-collections-v1.fixtures/invocation-set-label-invalid-dependency.json` | `1606ebb1b96f178d7ee291713eb1e8ad32752ea559854530559921b29e7c4cdb` |
| `fixtures/vqro-collections-v1/invocation-set-label-valid.json` | `docs/next/api/vqro-collections-v1.fixtures/invocation-set-label-valid.json` | `cd7342eb41ef1eac2ead785e1eb9deae5d0e74dcaef012ca0c0e9109c2b60cab` |
| `fixtures/vqro-collections-v1/labeled-create-atomic.json` | `docs/next/api/vqro-collections-v1.fixtures/labeled-create-atomic.json` | `12e52bcf4fae8aa160fc30c9f88d257e7f2fc7f465328ecb8a6430e5637b56e4` |
| `fixtures/vqro-collections-v1/old-reader-rejection-v7.json` | `docs/next/api/vqro-collections-v1.fixtures/old-reader-rejection-v7.json` | `43dd15edc9714593e5c715d7aadd9fff336318ca625b8d3925743a315fbe6bcc` |
| `fixtures/vqro-collections-v1/package-authority-valid.json` | `docs/next/api/vqro-collections-v1.fixtures/package-authority-valid.json` | `2e2e66172e9e83e6069590143536fa5ee66b68c6388e0a3eed7be7bb08866850` |
| `fixtures/vqro-collections-v1/projected-collections-structure-v1.json` | `docs/next/api/vqro-collections-v1.fixtures/projected-collections-structure-v1.json` | `23de8335df296eb59e7f351d45241a2595474285cfe53ec46116552096f8680f` |
| `fixtures/vqro-collections-v1/projected-host-document-v1.json` | `docs/next/api/vqro-collections-v1.fixtures/projected-host-document-v1.json` | `5bc8f5a2be7f6f1ad17601097f25d99209b049383211ed033dc20384a38d8389` |

`cargo run --locked -p xtask -- contract-check` verifies every listed local asset plus this provenance file and the terminal fingerprint vector. Packaging carries the same assets; `xtask verify` additionally binds the reviewed manifest, component, and whole-package digests. Contract equality does not grant host read leases, provider admission, publication, or feature authority.
