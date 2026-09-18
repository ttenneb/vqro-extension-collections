use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use wasmparser::{Parser, Payload, Validator, WasmFeatures};

const CONTRACT_FILES: [(&str, &str); 33] = [
    (
        "contracts/vqro-extension-service/world.wit",
        "625f909dcd26c714e94b0361e4c3fde969c792e0497aaf7477e08f4f73f22daf",
    ),
    (
        "contracts/api/vqro-service-v1.schema.json",
        "3c2712d8b92b4908a40b92360e7fe089e914a88b1cb07640dc8d5c5d7129bdbc",
    ),
    (
        "contracts/api/host-state-v1.schema.json",
        "76f18212c69780c5100594598a3aa388ddf70e4d275461ca1cbab9b2418f77bd",
    ),
    (
        "contracts/api/host-terminals-v1.schema.json",
        "3b0a9fff414d1f2b0ca57b8e26910d4da9ec11a5fe30a7704a4a3acc74680449",
    ),
    (
        "contracts/api/host-document-render-v2.schema.json",
        "a500fef482259ce90772f9e677c3b9a943d006cfd0a75abe7915cf1bc2994776",
    ),
    (
        "contracts/api/host-document-v2.schema.json",
        "1c433b3b25ca1703204ec51f3f65f02541f2fcd02568105d34da53731d8bef52",
    ),
    (
        "contracts/api/host-document-v2.md",
        "50acbfdb201b1a56006f47a8c6a6cdb20761b0bd1d6d8c7b0dd65a0ddf5d3832",
    ),
    (
        "contracts/fixtures/host-document-v2/valid.json",
        "95018a287650b5de06691b200a091643eb5130b0387918812fddadb07e016163",
    ),
    (
        "contracts/fixtures/host-document-v2/invalid-action.json",
        "10d6a80150e18d0b18d39e6c1f76ea0ed38a4d76f8d961f653a94d18520a637f",
    ),
    (
        "contracts/fixtures/host-document-v2/invalid-unknown-field.json",
        "2cf6c0d971e06925c553f893a30af0c33049d79b929204a5990199de0a15197f",
    ),
    (
        "contracts/fixtures/host-document-v2/invalid-unreachable.json",
        "020e74c6f94c63f9f6399d2deaf71956877edebb06dd943a18a98c0ba1a8e100",
    ),
    (
        "contracts/fixtures/host-terminals-v1/fingerprint-vector.json",
        "ace75ef459e96136563ee3cc747d0e2537a8f5cadd0f8af41a95f4ccf3597105",
    ),
    (
        "contracts/package/README.md",
        "ff3dc63baf00108dc465b30b5adc98d5317de58e494bb74c1cf20f67f741fef2",
    ),
    (
        "contracts/package/vqro-collections-action-invocation-v1.schema.json",
        "59ee8f1d359954428038c16f75b8999af7698c6185ed9b71dda806ea20d206e9",
    ),
    (
        "contracts/package/vqro-collections-actions-v1.schema.json",
        "3078eed5a4b876ad9bb7b462d290924b0b6527ec6be8e7f2c279286e66856f0b",
    ),
    (
        "contracts/package/vqro-collections-document-profile-v1.schema.json",
        "aa77062beecc223ebc6068ede0887406e584dbd1507cb9241eff0e0dd8e30b2a",
    ),
    (
        "contracts/package/vqro-collections-effect-plan-v1.schema.json",
        "5fadc52b3fff7df6532f6e9603defc27681c57cfe338bc442b5a7de7dbc5bc4f",
    ),
    (
        "contracts/package/vqro-collections-production-pin-v1.schema.json",
        "dc4bf74ee492067644db6f24bd39a878efc05afefa6dad39eaf84c9d32ea4923",
    ),
    (
        "contracts/fixtures/vqro-collections-v1/actions-valid.json",
        "d2c552b9d1afe194b3175e532d85e3747e5a6d1456d8e207a398e8f06b1a4f83",
    ),
    (
        "contracts/fixtures/vqro-collections-v1/cancellation-before-commit.json",
        "7a8ea24cc353e3948926e930b95e15652eb5be43c6d85e54bc172e8839fb4c98",
    ),
    (
        "contracts/fixtures/vqro-collections-v1/document-profile-invalid-root-label.json",
        "700c92514235523dd369dbea694224a985cc1e70ce57fc7207507726f1f0cf79",
    ),
    (
        "contracts/fixtures/vqro-collections-v1/document-profile-valid.json",
        "f7aa6ee90235f2dfca26c51cfdbf8cbf0983a8150e11a844494ab347c7b7e182",
    ),
    (
        "contracts/fixtures/vqro-collections-v1/effect-plan-invalid-ticket.json",
        "8dcc305725af91e0c11c34ec46ab75856c4c8f4ace0106cebcfb4525300e229b",
    ),
    (
        "contracts/fixtures/vqro-collections-v1/effect-plan-valid.json",
        "206b41ede9eea60a90bd95e6d1be005d2659a291ae265de898c48db92e5e8fd0",
    ),
    (
        "contracts/fixtures/vqro-collections-v1/invocation-set-archived-valid.json",
        "482dabd84f8d7fae2cd467b4c611fc009ae365312527a091dfcfaf41eb2fb0dc",
    ),
    (
        "contracts/fixtures/vqro-collections-v1/invocation-set-label-invalid-dependency.json",
        "1606ebb1b96f178d7ee291713eb1e8ad32752ea559854530559921b29e7c4cdb",
    ),
    (
        "contracts/fixtures/vqro-collections-v1/invocation-set-label-valid.json",
        "cd7342eb41ef1eac2ead785e1eb9deae5d0e74dcaef012ca0c0e9109c2b60cab",
    ),
    (
        "contracts/fixtures/vqro-collections-v1/labeled-create-atomic.json",
        "12e52bcf4fae8aa160fc30c9f88d257e7f2fc7f465328ecb8a6430e5637b56e4",
    ),
    (
        "contracts/fixtures/vqro-collections-v1/old-reader-rejection-v7.json",
        "43dd15edc9714593e5c715d7aadd9fff336318ca625b8d3925743a315fbe6bcc",
    ),
    (
        "contracts/fixtures/vqro-collections-v1/package-authority-valid.json",
        "2e2e66172e9e83e6069590143536fa5ee66b68c6388e0a3eed7be7bb08866850",
    ),
    (
        "contracts/fixtures/vqro-collections-v1/projected-collections-structure-v1.json",
        "23de8335df296eb59e7f351d45241a2595474285cfe53ec46116552096f8680f",
    ),
    (
        "contracts/fixtures/vqro-collections-v1/projected-host-document-v1.json",
        "5bc8f5a2be7f6f1ad17601097f25d99209b049383211ed033dc20384a38d8389",
    ),
    (
        "contracts/PROVENANCE.md",
        "8770c577c3a369219dd7f6b2671a87c2aa9af91a0f4e4834fd60baf2fd1f6b34",
    ),
];
#[cfg(test)]
const SOURCE_ONLY_PROFILE_FILES: [&str; 5] = [
    "profiles/README.md",
    "profiles/collections-policy-v1.md",
    "profiles/collections-policy-v1.schema.json",
    "profiles/fixtures/collections-policy-v1-invalid.json",
    "profiles/fixtures/collections-policy-v1.json",
];
const EXPECTED_PACKAGE_PATHS: [&str; 38] = [
    "LICENSE",
    "README.md",
    "checksums.sha256",
    "contracts/PROVENANCE.md",
    "contracts/api/host-document-render-v2.schema.json",
    "contracts/api/host-document-v2.md",
    "contracts/api/host-document-v2.schema.json",
    "contracts/api/host-state-v1.schema.json",
    "contracts/api/host-terminals-v1.schema.json",
    "contracts/api/vqro-service-v1.schema.json",
    "contracts/fixtures/host-document-v2/invalid-action.json",
    "contracts/fixtures/host-document-v2/invalid-unknown-field.json",
    "contracts/fixtures/host-document-v2/invalid-unreachable.json",
    "contracts/fixtures/host-document-v2/valid.json",
    "contracts/fixtures/host-terminals-v1/fingerprint-vector.json",
    "contracts/fixtures/vqro-collections-v1/actions-valid.json",
    "contracts/fixtures/vqro-collections-v1/cancellation-before-commit.json",
    "contracts/fixtures/vqro-collections-v1/document-profile-invalid-root-label.json",
    "contracts/fixtures/vqro-collections-v1/document-profile-valid.json",
    "contracts/fixtures/vqro-collections-v1/effect-plan-invalid-ticket.json",
    "contracts/fixtures/vqro-collections-v1/effect-plan-valid.json",
    "contracts/fixtures/vqro-collections-v1/invocation-set-archived-valid.json",
    "contracts/fixtures/vqro-collections-v1/invocation-set-label-invalid-dependency.json",
    "contracts/fixtures/vqro-collections-v1/invocation-set-label-valid.json",
    "contracts/fixtures/vqro-collections-v1/labeled-create-atomic.json",
    "contracts/fixtures/vqro-collections-v1/old-reader-rejection-v7.json",
    "contracts/fixtures/vqro-collections-v1/package-authority-valid.json",
    "contracts/fixtures/vqro-collections-v1/projected-collections-structure-v1.json",
    "contracts/fixtures/vqro-collections-v1/projected-host-document-v1.json",
    "contracts/package/README.md",
    "contracts/package/vqro-collections-action-invocation-v1.schema.json",
    "contracts/package/vqro-collections-actions-v1.schema.json",
    "contracts/package/vqro-collections-document-profile-v1.schema.json",
    "contracts/package/vqro-collections-effect-plan-v1.schema.json",
    "contracts/package/vqro-collections-production-pin-v1.schema.json",
    "contracts/vqro-extension-service/world.wit",
    "services/collections.wasm",
    "vqro-extension.toml",
];
const EXPECTED_PACKAGE_SHA256: &str =
    "067726c3ab83b44485a28926a1fd485a9ab05d1bd807382e6388fb5457299676";
const EXPECTED_MANIFEST_SHA256: &str =
    "34751e197b31206ed2834a6243cd5ae27ecb8e1b44a57bb6594d55f724209385";
const EXPECTED_COMPONENT_SHA256: &str =
    "a7f4c30b3d01298249a516f6d7f1d813e638c1383eff6435e70e72d6c55b65f7";
const MAX_COMPONENT_BYTES: usize = 512 * 1024;
const MAX_PACKAGE_BYTES: usize = 8 * 1024 * 1024;
const MAX_PACKAGE_FILE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_EXPANDED_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PACKAGE_ENTRIES: usize = 10_000;
const MAX_MANIFEST_BYTES: usize = 256 * 1024;
const MAX_CHECKSUM_BYTES: usize = 1024 * 1024;
const MAX_PATH_BYTES: usize = 512;
const MAX_PATH_COMPONENT_BYTES: usize = 128;
const MAX_PATH_DEPTH: usize = 16;
const PACKAGE_NAME: &str = "vqro-collections.vqrox";
const ALLOWED_COMPONENT_IMPORTS: [&str; 4] = [
    "service-descriptor",
    "service-error",
    "vqro:extension/host@1.0.0",
    "vqro:extension/types@1.0.0",
];

#[derive(Clone, Debug, Eq, PartialEq)]
struct Entry {
    path: String,
    bytes: Vec<u8>,
    mode: u32,
}

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("contract-check") if args.next().is_none() => contract_check(),
        Some("package") => {
            let output = args.next().map(PathBuf::from);
            if args.next().is_some() {
                bail!("usage: cargo run -p xtask -- package [output]");
            }
            package(output.as_deref())
        }
        Some("verify") => {
            let path = args
                .next()
                .map(PathBuf::from)
                .context("usage: cargo run -p xtask -- verify <package.vqrox>")?;
            if args.next().is_some() {
                bail!("usage: cargo run -p xtask -- verify <package.vqrox>");
            }
            verify_reviewed_package(&path)
        }
        _ => bail!("usage: cargo run -p xtask -- <contract-check|package|verify>"),
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must be a workspace member")
        .to_path_buf()
}

fn cargo_target_dir(root: &Path) -> PathBuf {
    match env::var_os("CARGO_TARGET_DIR").map(PathBuf::from) {
        Some(path) if path.is_absolute() => path,
        Some(path) => root.join(path),
        None => root.join("target"),
    }
}

fn contract_check() -> Result<()> {
    let root = workspace_root();
    for (relative, expected) in CONTRACT_FILES {
        let path = root.join(relative);
        let actual = sha256(&fs::read(&path).with_context(|| format!("read {}", path.display()))?);
        if actual != expected {
            bail!("contract hash mismatch for {relative}: expected {expected}, got {actual}");
        }
        println!("contract {relative} {actual}");
    }
    Ok(())
}

fn package(output: Option<&Path>) -> Result<()> {
    contract_check()?;
    let root = workspace_root();
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .current_dir(&root)
        .args([
            "build",
            "--locked",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
            "-p",
            "vqro-collections-component",
        ])
        .status()
        .context("run component build")?;
    if !status.success() {
        bail!("component build failed");
    }

    let core_path = cargo_target_dir(&root)
        .join("wasm32-unknown-unknown/release/vqro_collections_component.wasm");
    let core = fs::read(&core_path)
        .with_context(|| format!("read built core module {}", core_path.display()))?;
    verify_embedded_world(&core)?;
    let component = wit_component::ComponentEncoder::default()
        .validate(true)
        .module(&core)
        .context("read embedded component metadata")?
        .encode()
        .context("encode component")?;
    verify_component(&component)?;

    let mut payload = BTreeMap::new();
    payload.insert("LICENSE".to_string(), fs::read(root.join("LICENSE"))?);
    payload.insert("README.md".to_string(), fs::read(root.join("README.md"))?);
    for (relative, _) in CONTRACT_FILES {
        payload.insert(relative.to_string(), fs::read(root.join(relative))?);
    }
    payload.insert("services/collections.wasm".to_string(), component);
    payload.insert(
        "vqro-extension.toml".to_string(),
        fs::read(root.join("vqro-extension.toml"))?,
    );

    let checksums = checksum_manifest(&payload);
    payload.insert("checksums.sha256".to_string(), checksums);
    let entries = payload
        .into_iter()
        .map(|(path, bytes)| Entry {
            path,
            bytes,
            mode: 0o644,
        })
        .collect::<Vec<_>>();
    let archive = canonical_archive(&entries)?;
    if archive.len() > MAX_PACKAGE_BYTES {
        bail!("package exceeds the host 8 MiB archive limit");
    }

    let output = output
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("dist").join(PACKAGE_NAME));
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, &archive).with_context(|| format!("write {}", output.display()))?;
    validate_package_against_current_source(&output)?;
    println!("package {} sha256={}", output.display(), sha256(&archive));
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArtifactIdentity {
    CurrentSource,
    Reviewed,
}

fn validate_package_against_current_source(path: &Path) -> Result<()> {
    verify_package(path, ArtifactIdentity::CurrentSource)
}

fn verify_reviewed_package(path: &Path) -> Result<()> {
    verify_package(path, ArtifactIdentity::Reviewed)
}

fn verify_reviewed_digest(
    identity: ArtifactIdentity,
    artifact: &str,
    actual: &str,
    expected: &str,
) -> Result<()> {
    if identity == ArtifactIdentity::Reviewed && actual != expected {
        bail!("{artifact} digest mismatch: expected {expected}, got {actual}");
    }
    Ok(())
}

fn verify_package(path: &Path, identity: ArtifactIdentity) -> Result<()> {
    contract_check()?;
    let metadata = fs::metadata(path).with_context(|| format!("stat {}", path.display()))?;
    if metadata.len() > MAX_PACKAGE_BYTES as u64 {
        bail!("package exceeds the host 8 MiB archive limit");
    }
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let package_digest = sha256(&bytes);
    verify_reviewed_digest(
        identity,
        "package",
        &package_digest,
        EXPECTED_PACKAGE_SHA256,
    )?;
    let entries = read_archive(&bytes)?;
    if canonical_archive(&entries)? != bytes {
        bail!("package is not the canonical deterministic tar encoding");
    }

    let paths = entries
        .iter()
        .map(|entry| entry.path.as_str())
        .collect::<Vec<_>>();
    if paths != EXPECTED_PACKAGE_PATHS {
        bail!("unexpected package inventory: {paths:?}");
    }
    if entries.iter().any(|entry| entry.mode != 0o644) {
        bail!("candidate package files must all use mode 0644");
    }

    let by_path = entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry.bytes.as_slice()))
        .collect::<BTreeMap<_, _>>();
    if by_path["vqro-extension.toml"].len() > MAX_MANIFEST_BYTES
        || by_path["checksums.sha256"].len() > MAX_CHECKSUM_BYTES
    {
        bail!("manifest or checksum inventory exceeds its host limit");
    }
    let checksums = std::str::from_utf8(by_path["checksums.sha256"])?;
    let payload = entries
        .iter()
        .filter(|entry| entry.path != "checksums.sha256")
        .map(|entry| (entry.path.clone(), entry.bytes.clone()))
        .collect::<BTreeMap<_, _>>();
    if checksums.as_bytes() != checksum_manifest(&payload) {
        bail!("checksum inventory is not exact or canonical");
    }

    for (contract_path, expected_digest) in CONTRACT_FILES {
        let actual = sha256(by_path[contract_path]);
        if actual != expected_digest {
            bail!(
                "packaged contract digest mismatch for {contract_path}: expected {expected_digest}, got {actual}"
            );
        }
    }

    let manifest_bytes = by_path["vqro-extension.toml"];
    let manifest_digest = sha256(manifest_bytes);
    verify_reviewed_digest(
        identity,
        "manifest",
        &manifest_digest,
        EXPECTED_MANIFEST_SHA256,
    )?;
    let expected_manifest = fs::read(workspace_root().join("vqro-extension.toml"))?;
    if manifest_bytes != expected_manifest {
        bail!("package manifest differs from the reviewed candidate manifest");
    }
    let manifest = std::str::from_utf8(manifest_bytes)?;
    for required in [
        "manifest_version = 2",
        "package_contract = \"vqro.package.v1\"",
        "id = \"vqro.collections\"",
        "id = \"collections\"",
        "version = \"1.0.0\"",
        "min_vqro_version = \"0.9.0\"",
        "provides = [\"host_document\"]",
        "capabilities = [\"host.state.read\", \"host.terminals.read\"]",
        "kind = \"component\"",
        "world = \"vqro:extension/service@1.0.0\"",
    ] {
        if manifest.matches(required).count() != 1 {
            bail!("manifest must contain exactly one candidate declaration {required:?}");
        }
    }
    if manifest.matches("[[services]]").count() != 1
        || manifest.matches("[services.runtime]").count() != 1
    {
        bail!("manifest must declare exactly one service and runtime");
    }
    for forbidden in [
        "command =",
        "_extension-service",
        "focus_terminal",
        "host.state.write",
        "state.transact",
        "host.action",
        "host.effect",
        "activation",
        "wasi",
    ] {
        if manifest.contains(forbidden) {
            bail!("manifest contains forbidden authority or self-spawn marker {forbidden:?}");
        }
    }

    let component_bytes = by_path["services/collections.wasm"];
    let component_digest = sha256(component_bytes);
    verify_reviewed_digest(
        identity,
        "component",
        &component_digest,
        EXPECTED_COMPONENT_SHA256,
    )?;
    verify_component(component_bytes)?;
    println!("verified {} sha256={package_digest}", path.display());
    Ok(())
}

fn verify_component(bytes: &[u8]) -> Result<()> {
    if bytes.len() > MAX_COMPONENT_BYTES {
        bail!(
            "component exceeds {} byte candidate budget: {}",
            MAX_COMPONENT_BYTES,
            bytes.len()
        );
    }
    Validator::new_with_features(WasmFeatures::all())
        .validate_all(bytes)
        .context("validate WebAssembly component")?;

    match wit_component::decode(bytes).context("decode component world")? {
        wit_component::DecodedWasm::Component(..) => {}
        wit_component::DecodedWasm::WitPackage(..) => bail!("payload is WIT, not a component"),
    }

    let mut imports = BTreeSet::new();
    let mut exports = BTreeSet::new();
    let mut core_imports = BTreeSet::new();
    for payload in Parser::new(0).parse_all(bytes) {
        match payload? {
            Payload::ComponentImportSection(section) => {
                for import in section {
                    imports.insert(import?.name.0.to_string());
                }
            }
            Payload::ComponentExportSection(section) => {
                for export in section {
                    exports.insert(export?.name.0.to_string());
                }
            }
            Payload::ImportSection(section) => {
                for import in section.into_imports() {
                    let import = import?;
                    core_imports.insert(format!("{}::{}", import.module, import.name));
                }
            }
            _ => {}
        }
    }
    let expected = ALLOWED_COMPONENT_IMPORTS
        .into_iter()
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    if imports != expected {
        bail!("component import budget mismatch: expected {expected:?}, got {imports:?}");
    }
    if exports != BTreeSet::from(["descriptor".to_string(), "invoke".to_string()]) {
        bail!("component export budget mismatch: {exports:?}");
    }
    let expected_core_imports = BTreeSet::from([
        "::$imports".to_string(),
        "::0".to_string(),
        "vqro:extension/host@1.0.0::call".to_string(),
        "vqro:extension/host@1.0.0::cancelled".to_string(),
    ]);
    if core_imports != expected_core_imports {
        bail!(
            "component core lowering import budget mismatch: expected {expected_core_imports:?}, got {core_imports:?}"
        );
    }
    if !bytes
        .windows(b"collections".len())
        .any(|window| window == b"collections")
    {
        bail!("component is missing the collections service marker");
    }
    for marker in [
        b"vqro.collections.document-profile.render.v1".as_slice(),
        b"vqro.collections.actions.list.v1".as_slice(),
        b"vqro.collections.action.plan.v1".as_slice(),
        b"vqro.collections.document-profile.v1".as_slice(),
        b"vqro.collections.actions.v1".as_slice(),
        b"vqro.collections.action-invocation.v1".as_slice(),
        b"vqro.collections.effect-plan.v1".as_slice(),
        b"host.state.read".as_slice(),
        b"state.snapshot".as_slice(),
        b"host.terminals.read".as_slice(),
        b"terminals.snapshot".as_slice(),
    ] {
        if bytes
            .windows(marker.len())
            .filter(|window| *window == marker)
            .count()
            != 1
        {
            bail!("component must contain exactly one reviewed method/capability marker");
        }
    }
    for marker in [
        b"host.document.action.invoke".as_slice(),
        b"vqro.effect-plan.v1".as_slice(),
        b"state.cas".as_slice(),
        b"host.state.write".as_slice(),
        b"state.transact".as_slice(),
        b"host.action".as_slice(),
        b"host.effect".as_slice(),
        b"focus_terminal".as_slice(),
    ] {
        if bytes.windows(marker.len()).any(|window| window == marker) {
            bail!("component contains a forbidden authority marker");
        }
    }
    if bytes.windows(5).any(|window| window == b"wasi:") {
        bail!("component contains a forbidden WASI import");
    }
    Ok(())
}

fn verify_embedded_world(core: &[u8]) -> Result<()> {
    let (_, bindgen) =
        wit_component::metadata::decode(core).context("decode embedded WIT world")?;
    let exact_world = bindgen.resolve.worlds.iter().any(|(_, world)| {
        world.name == "service"
            && world.package.is_some_and(|id| {
                bindgen.resolve.packages[id].name.to_string() == "vqro:extension@1.0.0"
            })
    });
    if !exact_world {
        bail!("component metadata does not contain vqro:extension/service@1.0.0");
    }
    Ok(())
}

fn checksum_manifest(files: &BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    let mut output = String::new();
    for (path, bytes) in files {
        output.push_str(&sha256(bytes));
        output.push_str("  ");
        output.push_str(path);
        output.push('\n');
    }
    output.into_bytes()
}

fn canonical_archive(entries: &[Entry]) -> Result<Vec<u8>> {
    let mut previous = None;
    let mut bytes = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut bytes);
        builder.mode(tar::HeaderMode::Deterministic);
        for entry in entries {
            if previous.is_some_and(|path: &str| path >= entry.path.as_str()) {
                bail!("archive entries are not in strict bytewise order");
            }
            previous = Some(entry.path.as_str());
            let mut header = tar::Header::new_ustar();
            header.set_path(&entry.path)?;
            header.set_size(entry.bytes.len() as u64);
            header.set_mode(entry.mode);
            header.set_uid(0);
            header.set_gid(0);
            header.set_mtime(0);
            header.set_cksum();
            builder.append(&header, entry.bytes.as_slice())?;
        }
        builder.finish()?;
    }
    Ok(bytes)
}

fn read_archive(bytes: &[u8]) -> Result<Vec<Entry>> {
    if bytes.len() > MAX_PACKAGE_BYTES {
        bail!("package exceeds the host 8 MiB archive limit");
    }
    let mut archive = tar::Archive::new(Cursor::new(bytes));
    let mut output = Vec::new();
    let mut previous: Option<String> = None;
    let mut casefolded = BTreeSet::new();
    let mut expanded = 0_u64;
    for (index, item) in archive.entries()?.enumerate() {
        if index >= MAX_PACKAGE_ENTRIES {
            bail!("archive exceeds the host entry limit");
        }
        let mut item = item?;
        if !item.header().entry_type().is_file() {
            bail!("archive contains a non-file entry");
        }
        let path = item
            .path()?
            .to_str()
            .context("archive path is not UTF-8")?
            .to_string();
        validate_package_path(&path)?;
        if previous
            .as_deref()
            .is_some_and(|value| value >= path.as_str())
        {
            bail!("archive paths are not in strict bytewise order");
        }
        previous = Some(path.clone());
        if !casefolded.insert(path.to_ascii_lowercase()) {
            bail!("archive paths collide under ASCII case folding");
        }
        if output.iter().any(|entry: &Entry| {
            path.starts_with(&format!("{}/", entry.path))
                || entry.path.starts_with(&format!("{path}/"))
        }) {
            bail!("archive contains a file/descendant path collision");
        }
        if item.header().mtime()? != 0 || item.header().uid()? != 0 || item.header().gid()? != 0 {
            bail!("archive metadata is not deterministic");
        }
        let size = item.header().size()?;
        if size > MAX_PACKAGE_FILE_BYTES {
            bail!("archive file exceeds the host 8 MiB file limit");
        }
        expanded = expanded
            .checked_add(size)
            .context("expanded size overflow")?;
        if expanded > MAX_EXPANDED_BYTES {
            bail!("archive exceeds the host expanded-data limit");
        }
        let mode = item.header().mode()?;
        let mut contents = Vec::with_capacity(size as usize);
        item.read_to_end(&mut contents)?;
        if contents.len() as u64 != size {
            bail!("archive entry size does not match its contents");
        }
        output.push(Entry {
            path,
            bytes: contents,
            mode,
        });
    }
    Ok(output)
}

fn validate_package_path(path: &str) -> Result<()> {
    if path.is_empty()
        || path.len() > MAX_PATH_BYTES
        || path.starts_with('/')
        || path.contains('\\')
        || path.contains(':')
        || path
            .bytes()
            .any(|byte| byte == 0 || byte < 0x20 || byte == 0x7f)
    {
        bail!("archive contains a non-portable path {path:?}");
    }
    let parts = path.split('/').collect::<Vec<_>>();
    if parts.len() > MAX_PATH_DEPTH {
        bail!("archive path exceeds the host depth limit");
    }
    for part in parts {
        if part.is_empty()
            || matches!(part, "." | "..")
            || part.len() > MAX_PATH_COMPONENT_BYTES
            || part.ends_with('.')
            || part.ends_with(' ')
            || windows_reserved(part)
        {
            bail!("archive contains a non-portable path component {part:?}");
        }
    }
    Ok(())
}

fn windows_reserved(part: &str) -> bool {
    let stem = part
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
        || stem
            .strip_prefix("com")
            .or_else(|| stem.strip_prefix("lpt"))
            .is_some_and(|suffix| {
                matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            })
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_only_policy_profiles_are_present_and_excluded_from_package_inventory() {
        let root = workspace_root();
        for profile in SOURCE_ONLY_PROFILE_FILES {
            assert!(
                root.join(profile).is_file(),
                "missing source profile {profile}"
            );
            assert!(!CONTRACT_FILES.iter().any(|(path, _)| *path == profile));
            assert!(!EXPECTED_PACKAGE_PATHS.contains(&profile));
        }
        assert!(EXPECTED_PACKAGE_PATHS
            .iter()
            .all(|path| !path.starts_with("profiles/")));
    }

    #[test]
    fn current_source_validation_skips_only_reviewed_digest_binding() {
        for artifact in ["package", "component"] {
            verify_reviewed_digest(
                ArtifactIdentity::CurrentSource,
                artifact,
                "local",
                "reviewed",
            )
            .unwrap();
            assert!(verify_reviewed_digest(
                ArtifactIdentity::Reviewed,
                artifact,
                "local",
                "reviewed"
            )
            .is_err());
            verify_reviewed_digest(ArtifactIdentity::Reviewed, artifact, "reviewed", "reviewed")
                .unwrap();
        }
    }

    #[test]
    fn checksum_manifest_is_sorted_and_excludes_itself_by_construction() {
        let files = BTreeMap::from([
            ("z".to_string(), b"last".to_vec()),
            ("a".to_string(), b"first".to_vec()),
        ]);
        let actual = String::from_utf8(checksum_manifest(&files)).unwrap();
        assert_eq!(
            actual,
            format!("{}  a\n{}  z\n", sha256(b"first"), sha256(b"last"))
        );
    }

    #[test]
    fn canonical_tar_has_exact_metadata_and_round_trips() {
        let entries = vec![
            Entry {
                path: "a".into(),
                bytes: b"one".to_vec(),
                mode: 0o644,
            },
            Entry {
                path: "b/c".into(),
                bytes: b"two".to_vec(),
                mode: 0o644,
            },
        ];
        let first = canonical_archive(&entries).unwrap();
        let second = canonical_archive(&entries).unwrap();
        assert_eq!(first, second);
        assert_eq!(read_archive(&first).unwrap(), entries);
        assert_eq!(first.len() % 512, 0);
    }

    #[test]
    fn canonical_tar_rejects_unsorted_entries() {
        let entries = vec![
            Entry {
                path: "b".into(),
                bytes: Vec::new(),
                mode: 0o644,
            },
            Entry {
                path: "a".into(),
                bytes: Vec::new(),
                mode: 0o644,
            },
        ];
        assert!(canonical_archive(&entries).is_err());
    }

    #[test]
    fn package_limits_match_the_public_host_contract() {
        assert_eq!(MAX_PACKAGE_BYTES, 8 * 1024 * 1024);
        assert_eq!(MAX_PACKAGE_FILE_BYTES, 8 * 1024 * 1024);
        assert_eq!(MAX_EXPANDED_BYTES, 64 * 1024 * 1024);
        assert_eq!(MAX_PACKAGE_ENTRIES, 10_000);
        assert_eq!(MAX_PATH_BYTES, 512);
        assert_eq!(MAX_PATH_COMPONENT_BYTES, 128);
        assert_eq!(MAX_PATH_DEPTH, 16);
    }

    #[test]
    fn package_paths_are_portable_and_bounded() {
        for valid in ["LICENSE", "services/collections.wasm", "a/b-c_1.txt"] {
            validate_package_path(valid).unwrap();
        }
        for invalid in [
            "",
            "/absolute",
            "../parent",
            "a/./b",
            "a//b",
            "a\\b",
            "C:drive",
            "NUL.txt",
            "com1",
            "trailing.",
            "trailing ",
        ] {
            assert!(
                validate_package_path(invalid).is_err(),
                "accepted {invalid:?}"
            );
        }
    }

    #[test]
    fn oversized_archive_is_rejected_before_tar_parsing() {
        let bytes = vec![0; MAX_PACKAGE_BYTES + 1];
        assert!(read_archive(&bytes).is_err());
    }

    #[test]
    fn pinned_contract_hash_matches_snapshot() {
        contract_check().unwrap();
    }
}
