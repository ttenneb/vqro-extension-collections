use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use wasmparser::{Parser, Payload, Validator, WasmFeatures};

const CONTRACT_FILES: [(&str, &str); 21] = [
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
        "efef31440b91de8b05ff99ef857a301864b05a141f7e0c1c13fdc3fa4708515a",
    ),
    (
        "contracts/package/collections-document-action-v1.schema.json",
        "6f8e66d759fa61e2419d912bbb91f02440505a08ab7e3addd229bc70d7fd73a2",
    ),
    (
        "contracts/package/effect-plan-v1.schema.json",
        "e19814f1bb52fadbac5a18d4ccc662027b4a0abcd424151d16ebed07d2c4d7cb",
    ),
    (
        "contracts/package/collections-migration-v1.schema.json",
        "e3dc275dcc08d546c27837af4619f297f65a4a419771ff685af8280edc980ad5",
    ),
    (
        "contracts/fixtures/collections-actions/label-invocation.json",
        "096272d3f6cac1b895a8f5b14acf9d948364c28bffe800cf6ae960840936838f",
    ),
    (
        "contracts/fixtures/collections-actions/label-effect-plan.json",
        "b85e5bb40a9b32750bda8add8a46bd3226369bba7b1ae83bc1db51e8a15f2038",
    ),
    (
        "contracts/fixtures/collections-migration/legacy-input.json",
        "e0bcdfc38775520e5a7a1d1426d07518a6521544975f6f4344946ab8ac4177fd",
    ),
    (
        "contracts/fixtures/collections-migration/migration-plan.json",
        "fd8220ca6937b7d6aaa6b9c9720b897ce5d793777d42a118c4fd267e398e2abf",
    ),
    (
        "contracts/PROVENANCE.md",
        "51636642391755c68de1595d4b351eb0ea1bc7a1e32130d79df91bc7a8f5a1fb",
    ),
];
#[cfg(test)]
const SOURCE_ONLY_PROFILE_FILES: [&str; 7] = [
    "profiles/README.md",
    "profiles/collections-policy-v1.md",
    "profiles/collections-policy-v1.schema.json",
    "profiles/collections-migration-v1.md",
    "profiles/fixtures/collections-legacy-v1.json",
    "profiles/fixtures/collections-policy-v1-invalid.json",
    "profiles/fixtures/collections-policy-v1.json",
];
const EXPECTED_PACKAGE_PATHS: [&str; 26] = [
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
    "contracts/fixtures/collections-actions/label-effect-plan.json",
    "contracts/fixtures/collections-actions/label-invocation.json",
    "contracts/fixtures/collections-migration/legacy-input.json",
    "contracts/fixtures/collections-migration/migration-plan.json",
    "contracts/fixtures/host-document-v2/invalid-action.json",
    "contracts/fixtures/host-document-v2/invalid-unknown-field.json",
    "contracts/fixtures/host-document-v2/invalid-unreachable.json",
    "contracts/fixtures/host-document-v2/valid.json",
    "contracts/fixtures/host-terminals-v1/fingerprint-vector.json",
    "contracts/package/README.md",
    "contracts/package/collections-document-action-v1.schema.json",
    "contracts/package/collections-migration-v1.schema.json",
    "contracts/package/effect-plan-v1.schema.json",
    "contracts/vqro-extension-service/world.wit",
    "services/collections.wasm",
    "vqro-extension.toml",
];
const EXPECTED_PACKAGE_SHA256: &str =
    "d9d05c130f8b79e0e508269d3872bf7491733c07186d69c7d4bde7100e175859";
const EXPECTED_MANIFEST_SHA256: &str =
    "1e03800d55f56eb959e9ebdabe6c0cc4380377683c5e2cbf939e792ca8f90304";
const EXPECTED_COMPONENT_SHA256: &str =
    "afbc83795434cbd92438193a34f71566438220848c67baa8de52d58615bb865e";
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
const PACKAGE_NAME: &str = "vqro-collections-candidate.vqrox";
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
    if manifest_digest != EXPECTED_MANIFEST_SHA256 {
        bail!(
            "manifest digest mismatch: expected {EXPECTED_MANIFEST_SHA256}, got {manifest_digest}"
        );
    }
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
        "version = \"0.0.0\"",
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
        b"host.document.render".as_slice(),
        b"host.document.action.invoke".as_slice(),
        b"vqro.effect-plan.v1".as_slice(),
        b"state.cas".as_slice(),
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
