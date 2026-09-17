use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use wasmparser::{Parser, Payload, Validator, WasmFeatures};

const CONTRACT_SHA256: &str = "625f909dcd26c714e94b0361e4c3fde969c792e0497aaf7477e08f4f73f22daf";
const PROVENANCE_SHA256: &str = "e345b4813a0db01de0270a643e340486a04dad952b22c1e63f5e13afc6f25e55";
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
const ALLOWED_COMPONENT_IMPORTS: [&str; 3] = [
    "service-descriptor",
    "service-error",
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
            verify_package(&path)
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
    let path = workspace_root().join("wit/vqro-extension-service/world.wit");
    let actual = sha256(&fs::read(&path).with_context(|| format!("read {}", path.display()))?);
    if actual != CONTRACT_SHA256 {
        bail!("WIT snapshot hash mismatch: expected {CONTRACT_SHA256}, got {actual}");
    }
    let provenance_path = workspace_root().join("wit/vqro-extension-service/PROVENANCE.md");
    let provenance = sha256(
        &fs::read(&provenance_path)
            .with_context(|| format!("read {}", provenance_path.display()))?,
    );
    if provenance != PROVENANCE_SHA256 {
        bail!("WIT provenance hash mismatch: expected {PROVENANCE_SHA256}, got {provenance}");
    }
    println!("contract {actual} provenance {provenance}");
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
    verify_package(&output)?;
    println!("package {} sha256={}", output.display(), sha256(&archive));
    Ok(())
}

fn verify_package(path: &Path) -> Result<()> {
    contract_check()?;
    let metadata = fs::metadata(path).with_context(|| format!("stat {}", path.display()))?;
    if metadata.len() > MAX_PACKAGE_BYTES as u64 {
        bail!("package exceeds the host 8 MiB archive limit");
    }
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let entries = read_archive(&bytes)?;
    if canonical_archive(&entries)? != bytes {
        bail!("package is not the canonical deterministic tar encoding");
    }

    let paths = entries
        .iter()
        .map(|entry| entry.path.as_str())
        .collect::<Vec<_>>();
    let expected = [
        "LICENSE",
        "README.md",
        "checksums.sha256",
        "services/collections.wasm",
        "vqro-extension.toml",
    ];
    if paths != expected {
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

    let manifest_bytes = by_path["vqro-extension.toml"];
    let expected_manifest = fs::read(workspace_root().join("vqro-extension.toml"))?;
    if manifest_bytes != expected_manifest {
        bail!("package manifest differs from the reviewed candidate manifest");
    }
    let manifest = std::str::from_utf8(manifest_bytes)?;
    for required in [
        "manifest_version = 2",
        "package_contract = \"vqro.package.v1\"",
        "id = \"vqro.collections\"",
        "version = \"0.0.0\"",
        "min_vqro_version = \"0.9.0\"",
        "provides = []",
        "capabilities = []",
        "kind = \"component\"",
        "world = \"vqro:extension/service@1.0.0\"",
    ] {
        if !manifest.contains(required) {
            bail!("manifest is missing exact candidate declaration {required:?}");
        }
    }
    for forbidden in [
        "command =",
        "_extension-service",
        "host_document",
        "focus_terminal",
    ] {
        if manifest.contains(forbidden) {
            bail!("manifest contains forbidden authority or self-spawn marker {forbidden:?}");
        }
    }

    verify_component(by_path["services/collections.wasm"])?;
    println!("verified {} sha256={}", path.display(), sha256(&bytes));
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
    let mut core_imports = Vec::new();
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
                    core_imports.push(format!("{}::{}", import.module, import.name));
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
    if !core_imports.is_empty() {
        bail!("component contains unexpected core imports: {core_imports:?}");
    }
    for marker in [b"collections".as_slice(), b"no_authority".as_slice()] {
        if !bytes.windows(marker.len()).any(|window| window == marker) {
            bail!("component is missing expected zero-authority marker");
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
