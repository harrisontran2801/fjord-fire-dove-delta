use crate::util::{file_size, sha256_file, MAX_INSPECT_BYTES, MAX_METADATA_BYTES};
use flate2::read::GzDecoder;
use serde::Serialize;
use serde_json::json;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub const UNSUPPORTED_FORMAT: &str =
    "Unsupported format. Supported formats: ELF binary, Docker save, OCI archive.";
pub const TOO_LARGE: &str = "File is too large to inspect. Maximum size is 96 MB.";
pub const READ_FAILED: &str = "Could not read this file. Check that it is readable and try again.";

#[derive(Debug, Clone, Serialize)]
pub struct ElfFacts {
    #[serde(rename = "classBits")]
    pub class_bits: u8,
    pub endian: String,
    pub machine: String,
    #[serde(rename = "machineCode")]
    pub machine_code: u16,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchiveFacts {
    pub format: String,
    #[serde(rename = "entryCount")]
    pub entry_count: usize,
    pub names: Vec<String>,
    #[serde(rename = "repoTags", skip_serializing_if = "Option::is_none")]
    pub repo_tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub architecture: Option<String>,
    #[serde(rename = "layerCount", skip_serializing_if = "Option::is_none")]
    pub layer_count: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct InspectOk {
    pub kind: String,
    pub name: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub elf: Option<ElfFacts>,
    pub archive: Option<ArchiveFacts>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum InspectOutcome {
    Ok(InspectOk),
    Err(String),
}

const ELF_MACHINES: &[(u16, &str)] = &[
    (0, "unspecified"),
    (3, "x86"),
    (8, "MIPS"),
    (20, "PowerPC"),
    (21, "PowerPC64"),
    (22, "S390"),
    (40, "ARM"),
    (42, "SuperH"),
    (50, "IA-64"),
    (62, "x86-64"),
    (183, "AArch64"),
    (243, "RISC-V"),
];

fn machine_name(code: u16) -> String {
    ELF_MACHINES
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, n)| (*n).to_string())
        .unwrap_or_else(|| format!("machine-{code}"))
}

fn u16_at(bytes: &[u8], offset: usize, little: bool) -> Option<u16> {
    let a = *bytes.get(offset)?;
    let b = *bytes.get(offset + 1)?;
    Some(if little {
        u16::from(a) | (u16::from(b) << 8)
    } else {
        (u16::from(a) << 8) | u16::from(b)
    })
}

pub fn parse_elf(bytes: &[u8]) -> Option<ElfFacts> {
    if bytes.len() < 20 || bytes[..4] != [0x7f, b'E', b'L', b'F'] {
        return None;
    }
    let class_byte = bytes[4];
    let data_byte = bytes[5];
    if class_byte != 1 && class_byte != 2 {
        return None;
    }
    if data_byte != 1 && data_byte != 2 {
        return None;
    }
    let little = data_byte == 1;
    let machine_code = u16_at(bytes, 18, little)?;
    Some(ElfFacts {
        class_bits: if class_byte == 1 { 32 } else { 64 },
        endian: if little { "little" } else { "big" }.to_string(),
        machine: machine_name(machine_code),
        machine_code,
    })
}

fn latin1(bytes: &[u8]) -> String {
    let mut out = String::new();
    for &c in bytes {
        if c == 0 {
            break;
        }
        out.push(char::from(c));
    }
    out
}

fn parse_octal(raw: &str) -> u64 {
    let trimmed = raw.trim_matches('\0').trim();
    if trimmed.is_empty() {
        return 0;
    }
    u64::from_str_radix(trimmed, 8).unwrap_or(0)
}

struct TarEntry {
    name: String,
    size: u64,
    data: Option<Vec<u8>>,
}

fn looks_like_tar_header(header: &[u8]) -> bool {
    if header.len() < 512 {
        return false;
    }
    let magic = latin1(&header[257..263.min(header.len())]);
    if magic == "ustar" {
        return true;
    }
    let size_field = latin1(&header[124..136]).trim().to_string();
    if !size_field.chars().all(|c| ('0'..='7').contains(&c)) || size_field.is_empty() {
        return false;
    }
    let name = latin1(&header[0..100]);
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_graphic() || c == ' ' || c == '/')
}

fn parse_tar<R: Read>(mut reader: R, load_meta: bool) -> Result<Vec<TarEntry>, String> {
    let mut entries = Vec::new();
    let mut header = [0u8; 512];
    loop {
        let mut read = 0;
        while read < 512 {
            let n = reader
                .read(&mut header[read..])
                .map_err(|e| e.to_string())?;
            if n == 0 {
                if read == 0 {
                    return Ok(entries);
                }
                return Err("truncated tar".into());
            }
            read += n;
        }
        if header.iter().all(|b| *b == 0) {
            break;
        }
        if !looks_like_tar_header(&header) && entries.is_empty() {
            return Err("not a tar archive".into());
        }
        let name = latin1(&header[0..100]);
        let prefix = latin1(&header[345..500]);
        let full = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let size = parse_octal(&latin1(&header[124..136]));
        let padded = ((size + 511) / 512) * 512;
        let basename = full.rsplit('/').next().unwrap_or(&full).to_string();
        let want = load_meta
            && size > 0
            && size as usize <= MAX_METADATA_BYTES
            && matches!(
                basename.as_str(),
                "manifest.json" | "oci-layout" | "index.json"
            )
            || (load_meta
                && size > 0
                && size as usize <= MAX_METADATA_BYTES
                && basename.ends_with(".json"));
        let mut data = None;
        if want {
            let mut buf = vec![0u8; size as usize];
            reader.read_exact(&mut buf).map_err(|e| e.to_string())?;
            data = Some(buf);
            let rest = (padded - size) as usize;
            if rest > 0 {
                let mut skip = vec![0u8; rest];
                reader.read_exact(&mut skip).map_err(|e| e.to_string())?;
            }
        } else {
            skip_bytes(&mut reader, padded)?;
        }
        if !full.is_empty() {
            entries.push(TarEntry {
                name: full.trim_start_matches("./").to_string(),
                size,
                data,
            });
        }
        if entries.len() > 8000 {
            break;
        }
    }
    Ok(entries)
}

fn skip_bytes<R: Read>(reader: &mut R, mut n: u64) -> Result<(), String> {
    let mut buf = [0u8; 8192];
    while n > 0 {
        let chunk = (n as usize).min(buf.len());
        let got = reader.read(&mut buf[..chunk]).map_err(|e| e.to_string())?;
        if got == 0 {
            return Err("truncated tar payload".into());
        }
        n -= got as u64;
    }
    Ok(())
}

fn is_docker_manifest(value: &serde_json::Value) -> bool {
    let Some(arr) = value.as_array() else {
        return false;
    };
    let Some(first) = arr.first() else {
        return false;
    };
    first.get("Layers").is_some()
        || first.get("RepoTags").is_some()
        || first.get("Config").is_some()
}

fn is_oci_layout(value: &serde_json::Value) -> bool {
    value.get("imageLayoutVersion").is_some()
}

fn is_oci_index(value: &serde_json::Value) -> bool {
    value.get("schemaVersion").and_then(|v| v.as_u64()) == Some(2)
        && value.get("manifests").and_then(|v| v.as_array()).is_some()
}

fn parse_json_bytes(bytes: Option<&Vec<u8>>) -> Option<serde_json::Value> {
    let bytes = bytes?;
    serde_json::from_slice(bytes).ok()
}

fn inspect_tar_entries(entries: &[TarEntry]) -> Option<(String, ArchiveFacts)> {
    if entries.is_empty() {
        return None;
    }
    let names: Vec<String> = entries.iter().map(|e| e.name.clone()).collect();
    let by_base: std::collections::HashMap<String, &TarEntry> = entries
        .iter()
        .map(|e| (e.name.rsplit('/').next().unwrap_or(&e.name).to_string(), e))
        .collect();

    let layout = parse_json_bytes(by_base.get("oci-layout").and_then(|e| e.data.as_ref()));
    let index = parse_json_bytes(by_base.get("index.json").and_then(|e| e.data.as_ref()));
    let has_blobs = names
        .iter()
        .any(|n| n.starts_with("blobs/") || n.contains("/blobs/"));
    if layout.as_ref().is_some_and(is_oci_layout)
        || index.as_ref().is_some_and(is_oci_index)
        || (by_base.contains_key("index.json") && has_blobs)
    {
        let layer_count = names.iter().filter(|n| n.contains("blobs/sha256/")).count();
        return Some((
            "oci".into(),
            ArchiveFacts {
                format: "oci".into(),
                entry_count: entries.len(),
                names: names.into_iter().take(16).collect(),
                repo_tags: None,
                architecture: None,
                layer_count: if layer_count == 0 {
                    None
                } else {
                    Some(layer_count)
                },
            },
        ));
    }

    let manifest = parse_json_bytes(by_base.get("manifest.json").and_then(|e| e.data.as_ref()));
    if let Some(manifest) = manifest.filter(is_docker_manifest) {
        let mut repo_tags = Vec::new();
        let mut layer_count = 0usize;
        if let Some(arr) = manifest.as_array() {
            for item in arr {
                if let Some(tags) = item.get("RepoTags").and_then(|v| v.as_array()) {
                    for t in tags {
                        if let Some(s) = t.as_str() {
                            repo_tags.push(s.to_string());
                        }
                    }
                }
                if let Some(layers) = item.get("Layers").and_then(|v| v.as_array()) {
                    layer_count += layers.len();
                }
            }
        }
        let mut architecture = None;
        if let Some(config_name) = manifest
            .as_array()
            .and_then(|a| a.first())
            .and_then(|i| i.get("Config"))
            .and_then(|c| c.as_str())
        {
            let cfg = entries.iter().find(|e| {
                e.name == config_name
                    || e.name.rsplit('/').next()
                        == Path::new(config_name).file_name().and_then(|s| s.to_str())
            });
            if let Some(value) = parse_json_bytes(cfg.and_then(|e| e.data.as_ref())) {
                architecture = value
                    .get("architecture")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            }
        }
        return Some((
            "docker".into(),
            ArchiveFacts {
                format: "docker-save".into(),
                entry_count: entries.len(),
                names: names.into_iter().take(16).collect(),
                repo_tags: if repo_tags.is_empty() {
                    None
                } else {
                    Some(repo_tags)
                },
                architecture,
                layer_count: if layer_count == 0 {
                    None
                } else {
                    Some(layer_count)
                },
            },
        ));
    }
    None
}

fn recommendations(kind: &str) -> Vec<String> {
    match kind {
        "elf" => vec![
            "ELF identified from magic bytes. Optimization requires a quench.yaml with build, test, and benchmark commands.".into(),
            "llvm-bolt is only applied when a valid profile exists and the tool is installed.".into(),
        ],
        "docker" | "oci" => vec![
            "Archive inspected only. Image layers were not rewritten.".into(),
            "To build a candidate image, provide a Dockerfile plus build and test commands. Docker/OCI rewrite is not attempted without those inputs.".into(),
        ],
        _ => vec![],
    }
}

pub fn inspect_path(path: &Path) -> InspectOutcome {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("artifact")
        .to_string();
    match inspect_path_inner(path, &name) {
        Ok(ok) => InspectOutcome::Ok(ok),
        Err(e) => InspectOutcome::Err(e),
    }
}

fn inspect_path_inner(path: &Path, name: &str) -> Result<InspectOk, String> {
    let size = file_size(path)?;
    if size > MAX_INSPECT_BYTES {
        return Err(TOO_LARGE.into());
    }
    let mut header = [0u8; 64];
    let mut file = File::open(path).map_err(|_| READ_FAILED.to_string())?;
    let n = file
        .read(&mut header)
        .map_err(|_| READ_FAILED.to_string())?;
    let header = &header[..n];
    if header.len() >= 2 && header[0] == 0x4d && header[1] == 0x5a {
        return Err(UNSUPPORTED_FORMAT.into());
    }
    if header.len() >= 8 && header[..8] == [0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1] {
        return Err(UNSUPPORTED_FORMAT.into());
    }

    let sha256 = sha256_file(path)?;

    if header.len() >= 4 && header[..4] == [0x7f, b'E', b'L', b'F'] {
        let elf = parse_elf(header).ok_or_else(|| UNSUPPORTED_FORMAT.to_string())?;
        return Ok(InspectOk {
            kind: "elf".into(),
            name: name.to_string(),
            size_bytes: size,
            sha256,
            elf: Some(elf),
            archive: None,
            recommendations: recommendations("elf"),
        });
    }

    file.seek(SeekFrom::Start(0))
        .map_err(|_| READ_FAILED.to_string())?;
    let gzip = header.len() >= 2 && header[0] == 0x1f && header[1] == 0x8b;
    let entries = if gzip {
        let decoder = GzDecoder::new(file);
        parse_tar(decoder, true).map_err(|_| UNSUPPORTED_FORMAT.to_string())?
    } else {
        parse_tar(file, true).map_err(|_| UNSUPPORTED_FORMAT.to_string())?
    };
    let Some((kind, archive)) = inspect_tar_entries(&entries) else {
        return Err(UNSUPPORTED_FORMAT.into());
    };
    Ok(InspectOk {
        recommendations: recommendations(&kind),
        kind,
        name: name.to_string(),
        size_bytes: size,
        sha256,
        elf: None,
        archive: Some(archive),
    })
}

pub fn inspect_to_json(outcome: &InspectOutcome) -> serde_json::Value {
    match outcome {
        InspectOutcome::Ok(ok) => {
            let mut v = json!({
                "ok": true,
                "kind": ok.kind,
                "name": ok.name,
                "sizeBytes": ok.size_bytes,
                "sha256": ok.sha256,
                "recommendations": ok.recommendations,
            });
            if let Some(elf) = &ok.elf {
                v["elf"] = serde_json::to_value(elf).unwrap_or(json!({}));
            }
            if let Some(archive) = &ok.archive {
                v["archive"] = serde_json::to_value(archive).unwrap_or(json!({}));
            }
            v
        }
        InspectOutcome::Err(err) => json!({ "ok": false, "error": err }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_tmp(bytes: &[u8], suffix: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("quench-inspect-{}-{suffix}", std::process::id()));
        let mut f = File::create(&path).unwrap();
        f.write_all(bytes).unwrap();
        path
    }

    fn tar_header(name: &str, size: usize) -> Vec<u8> {
        let mut header = vec![0u8; 512];
        let nb = name.as_bytes();
        header[..nb.len().min(99)].copy_from_slice(&nb[..nb.len().min(99)]);
        let size_oct = format!("{size:011o}");
        header[124..135].copy_from_slice(size_oct.as_bytes());
        header[156] = b'0';
        header[257..262].copy_from_slice(b"ustar");
        header[263] = b'0';
        header[264] = b'0';
        header[148..156].fill(b' ');
        let sum: u32 = header.iter().map(|b| *b as u32).sum();
        let chk = format!("{sum:06o}\0 ");
        header[148..156].copy_from_slice(chk.as_bytes());
        header
    }

    fn build_tar(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        for (name, content) in files {
            out.extend(tar_header(name, content.len()));
            out.extend_from_slice(content);
            let pad = (512 - (content.len() % 512)) % 512;
            out.extend(std::iter::repeat(0).take(pad));
        }
        out.extend(std::iter::repeat(0).take(1024));
        out
    }

    fn elf_bytes(payload: u8) -> Vec<u8> {
        let mut b = vec![0u8; 256];
        b[0..7].copy_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1, 1]);
        b[18] = 62;
        b[40] = payload;
        b
    }

    #[test]
    fn rejects_msi() {
        let mut bytes = vec![0u8; 4096];
        bytes[..8].copy_from_slice(&[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]);
        let path = write_tmp(&bytes, "setup.msi");
        match inspect_path(&path) {
            InspectOutcome::Err(e) => assert_eq!(e, UNSUPPORTED_FORMAT),
            _ => panic!("msi should be rejected"),
        }
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn rejects_exe() {
        let mut bytes = vec![0u8; 512];
        bytes[0] = 0x4d;
        bytes[1] = 0x5a;
        let path = write_tmp(&bytes, "app.exe");
        match inspect_path(&path) {
            InspectOutcome::Err(e) => assert_eq!(e, UNSUPPORTED_FORMAT),
            _ => panic!("exe should be rejected"),
        }
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn rejects_random_large() {
        let bytes = vec![0x41u8; 12_000];
        let path = write_tmp(&bytes, "docker-image.container.tar");
        assert!(matches!(inspect_path(&path), InspectOutcome::Err(_)));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn identifies_elf_and_hashes() {
        let path = write_tmp(&elf_bytes(7), "match-engine");
        match inspect_path(&path) {
            InspectOutcome::Ok(ok) => {
                assert_eq!(ok.kind, "elf");
                assert_eq!(ok.sha256.len(), 64);
                assert_eq!(ok.elf.as_ref().unwrap().machine, "x86-64");
            }
            InspectOutcome::Err(e) => panic!("{e}"),
        }
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn rejects_bad_elf_header() {
        let mut bytes = elf_bytes(1);
        bytes[4] = 99;
        let path = write_tmp(&bytes, "bad-elf");
        match inspect_path(&path) {
            InspectOutcome::Err(e) => assert_eq!(e, UNSUPPORTED_FORMAT),
            _ => panic!("bad elf header should fail"),
        }
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn identifies_docker_save() {
        let manifest = br#"[{"Config":"config.json","RepoTags":["ghcr.io/acme/api:1"],"Layers":["layer/layer.tar"]}]"#;
        let config = br#"{"architecture":"amd64","os":"linux"}"#;
        let tar = build_tar(&[
            ("manifest.json", manifest.as_ref()),
            ("config.json", config.as_ref()),
            ("layer/layer.tar", &[1, 2, 3, 4]),
        ]);
        let path = write_tmp(&tar, "api.tar");
        match inspect_path(&path) {
            InspectOutcome::Ok(ok) => {
                assert_eq!(ok.kind, "docker");
                assert_eq!(ok.archive.as_ref().unwrap().format, "docker-save");
                assert_eq!(
                    ok.archive.as_ref().unwrap().repo_tags.as_ref().unwrap()[0],
                    "ghcr.io/acme/api:1"
                );
            }
            InspectOutcome::Err(e) => panic!("{e}"),
        }
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn identifies_oci() {
        let tar = build_tar(&[
            ("oci-layout", br#"{"imageLayoutVersion":"1.0.0"}"#.as_ref()),
            (
                "index.json",
                br#"{"schemaVersion":2,"manifests":[{"mediaType":"application/vnd.oci.image.manifest.v1+json","digest":"sha256:abc"}]}"#.as_ref(),
            ),
            ("blobs/sha256/abc", &[9, 8, 7]),
        ]);
        let path = write_tmp(&tar, "image.oci.tar");
        match inspect_path(&path) {
            InspectOutcome::Ok(ok) => {
                assert_eq!(ok.kind, "oci");
                assert_eq!(ok.archive.as_ref().unwrap().format, "oci");
            }
            InspectOutcome::Err(e) => panic!("{e}"),
        }
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn different_contents_different_hash() {
        let a = write_tmp(&elf_bytes(1), "worker-a");
        let b = write_tmp(&elf_bytes(2), "worker-b");
        let ra = inspect_path(&a);
        let rb = inspect_path(&b);
        match (ra, rb) {
            (InspectOutcome::Ok(a), InspectOutcome::Ok(b)) => {
                assert_ne!(a.sha256, b.sha256);
                assert_eq!(a.size_bytes, b.size_bytes);
            }
            _ => panic!("both should inspect"),
        }
        let _ = std::fs::remove_file(a);
        let _ = std::fs::remove_file(b);
    }
}
