use crate::util::which;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

pub const LBR_PROBE_COMMAND: &str = "perf record -e cycles:u -j any,u -- sleep 0.3";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileMode {
    Lbr,
    Instrument,
    Nl,
    Unavailable,
}

impl Default for ProfileMode {
    fn default() -> Self {
        Self::Unavailable
    }
}

impl ProfileMode {
    pub fn as_str(self) -> &'static str {
        match self {
            ProfileMode::Lbr => "lbr",
            ProfileMode::Instrument => "instrument",
            ProfileMode::Nl => "nl",
            ProfileMode::Unavailable => "unavailable",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "lbr" => Some(ProfileMode::Lbr),
            "instrument" => Some(ProfileMode::Instrument),
            "nl" => Some(ProfileMode::Nl),
            "unavailable" | "none" => Some(ProfileMode::Unavailable),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileInfo {
    pub mode: ProfileMode,
    pub reason: String,
    pub lbr_probe_ok: Option<bool>,
    pub lbr_probe_command: String,
    pub lbr_probe_detail: String,
    pub bolt_rt: Option<String>,
    pub has_text_relocs: bool,
    pub fdata_path: Option<String>,
    pub instrumented_path: Option<String>,
    pub benchmarked_original: bool,
    pub benchmarked_candidate: bool,
    pub benchmarked_instrumented: bool,
    pub warning: Option<String>,
}

impl Default for ProfileInfo {
    fn default() -> Self {
        Self {
            mode: ProfileMode::Unavailable,
            reason: "No profile was collected".into(),
            lbr_probe_ok: None,
            lbr_probe_command: LBR_PROBE_COMMAND.into(),
            lbr_probe_detail: String::new(),
            bolt_rt: None,
            has_text_relocs: false,
            fdata_path: None,
            instrumented_path: None,
            benchmarked_original: false,
            benchmarked_candidate: false,
            benchmarked_instrumented: false,
            warning: None,
        }
    }
}

impl ProfileInfo {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "profileMode": self.mode.as_str(),
            "profileReason": self.reason,
            "lbrProbeOk": self.lbr_probe_ok,
            "lbrProbeCommand": self.lbr_probe_command,
            "lbrProbeDetail": self.lbr_probe_detail,
            "boltRuntimeLib": self.bolt_rt,
            "hasTextRelocs": self.has_text_relocs,
            "fdataPath": self.fdata_path,
            "instrumentedPath": self.instrumented_path,
            "benchmarkedOriginal": self.benchmarked_original,
            "benchmarkedCandidate": self.benchmarked_candidate,
            "benchmarkedInstrumented": self.benchmarked_instrumented,
            "benchmarkUsedUninstrumentedBinaries": self.benchmarked_original && !self.benchmarked_instrumented,
            "profileWarning": self.warning,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ProfileInputs {
    pub lbr_probe_ok: bool,
    pub perf_ok: bool,
    pub bolt_ok: bool,
    pub bolt_rt: Option<PathBuf>,
    pub has_text_relocs: bool,
    pub has_profile_cmd: bool,
    pub force: Option<ProfileMode>,
}

#[derive(Debug, Clone)]
pub struct ProfileDecision {
    pub mode: ProfileMode,
    pub reason: String,
    pub warning: Option<String>,
}

pub fn forced_profile_mode() -> Option<ProfileMode> {
    std::env::var("QUENCH_FORCE_PROFILE_MODE")
        .ok()
        .and_then(|s| ProfileMode::parse(&s))
}

pub fn select_profile_mode(input: ProfileInputs) -> ProfileDecision {
    if let Some(mode) = input.force {
        return ProfileDecision {
            mode,
            reason: format!("profile mode forced to {}", mode.as_str()),
            warning: instrumentation_warning(mode),
        };
    }
    if !input.bolt_ok {
        return ProfileDecision {
            mode: ProfileMode::Unavailable,
            reason: "llvm-bolt is Unavailable; no layout rewrite will be performed".into(),
            warning: None,
        };
    }
    if !input.has_profile_cmd {
        return ProfileDecision {
            mode: ProfileMode::Unavailable,
            reason: "No profile command in quench.yaml; no BOLT profile was collected".into(),
            warning: None,
        };
    }
    if input.lbr_probe_ok && input.perf_ok {
        return ProfileDecision {
            mode: ProfileMode::Lbr,
            reason: "LBR probe succeeded; collecting a branch-stack profile".into(),
            warning: None,
        };
    }
    if input.bolt_rt.is_some() {
        let mut warning = instrumentation_warning(ProfileMode::Instrument);
        if !input.has_text_relocs {
            warning = concat_warning(
                warning,
                "Binary has no .rela.text; rebuild with -Wl,--emit-relocs for BOLT.",
            );
        }
        return ProfileDecision {
            mode: ProfileMode::Instrument,
            reason:
                "LBR unavailable; using BOLT instrumentation on a copy (not production traffic)"
                    .into(),
            warning,
        };
    }
    ProfileDecision {
        mode: ProfileMode::Unavailable,
        reason: "No LBR and no libbolt_rt_instr.a; no profile was collected".into(),
        warning: None,
    }
}

fn instrumentation_warning(mode: ProfileMode) -> Option<String> {
    if mode == ProfileMode::Instrument {
        Some("BOLT instrumentation slows the copy used only for profiling. The instrumented binary is never benchmarked. Final numbers come from the uninstrumented original and candidate. This is not production traffic sampling.".into())
    } else {
        None
    }
}

fn concat_warning(base: Option<String>, extra: &str) -> Option<String> {
    Some(match base {
        Some(b) => format!("{b} {extra}"),
        None => extra.into(),
    })
}

pub struct LbrProbe {
    pub ok: bool,
    pub command: String,
    pub detail: String,
}

pub fn probe_lbr() -> LbrProbe {
    let command = LBR_PROBE_COMMAND.to_string();
    if let Ok(v) = std::env::var("QUENCH_FORCE_LBR") {
        let ok = v == "1" || v.eq_ignore_ascii_case("true");
        return LbrProbe {
            ok,
            command,
            detail: format!("QUENCH_FORCE_LBR={v}"),
        };
    }
    if which("perf").is_none() {
        return LbrProbe {
            ok: false,
            command,
            detail: "perf not found on PATH".into(),
        };
    }
    let dir = std::env::temp_dir().join(format!(
        "quench-lbr-probe-{}-{}",
        std::process::id(),
        crate::util::now_ms()
    ));
    let _ = std::fs::create_dir_all(&dir);
    let out = dir.join("lbr.data");
    let cmdline = format!(
        "perf record -e cycles:u -j any,u -o {} -- sleep 0.3",
        out.display()
    );
    let result = Command::new("sh").arg("-c").arg(&cmdline).output();
    let _ = std::fs::remove_dir_all(&dir);
    match result {
        Ok(outp) => {
            let stderr = String::from_utf8_lossy(&outp.stderr);
            let stdout = String::from_utf8_lossy(&outp.stdout);
            let ok = outp.status.success();
            let detail = if ok {
                "LBR probe succeeded".into()
            } else {
                let msg = format!("{stderr}{stdout}");
                let lines: Vec<&str> = msg
                    .lines()
                    .map(|l| l.trim())
                    .filter(|l| !l.is_empty() && *l != "Error:")
                    .collect();
                if lines.is_empty() {
                    "perf LBR probe failed".into()
                } else {
                    lines.into_iter().take(3).collect::<Vec<_>>().join(" ")
                }
            };
            LbrProbe {
                ok,
                command,
                detail,
            }
        }
        Err(e) => LbrProbe {
            ok: false,
            command,
            detail: format!("failed to spawn perf: {e}"),
        },
    }
}

pub fn find_bolt_rt_instr() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("QUENCH_BOLT_RT_INSTR") {
        let path = PathBuf::from(p);
        if path.is_file() {
            return Some(path);
        }
        return None;
    }
    let mut cands: Vec<PathBuf> = vec![
        PathBuf::from("/usr/local/lib/libbolt_rt_instr.a"),
        PathBuf::from("/usr/lib/libbolt_rt_instr.a"),
        PathBuf::from("/usr/lib/llvm-19/lib/libbolt_rt_instr.a"),
        PathBuf::from("/usr/lib/llvm-18/lib/libbolt_rt_instr.a"),
        PathBuf::from("/usr/lib/llvm-17/lib/libbolt_rt_instr.a"),
    ];
    if let Some(bolt) = which("llvm-bolt") {
        let resolved = std::fs::canonicalize(&bolt).unwrap_or_else(|_| PathBuf::from(&bolt));
        if let Some(bin_dir) = resolved.parent() {
            cands.push(bin_dir.join("../lib/libbolt_rt_instr.a"));
            if let Some(llvm_root) = bin_dir.parent() {
                cands.push(llvm_root.join("lib/libbolt_rt_instr.a"));
            }
        }
    }
    cands.into_iter().find(|p| {
        std::fs::canonicalize(p)
            .map(|c| c.is_file())
            .unwrap_or_else(|_| p.is_file())
    })
}

pub fn elf_has_text_relocs(path: &Path) -> Result<bool, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    if bytes.len() < 64 || bytes[0..4] != [0x7f, b'E', b'L', b'F'] {
        return Err("not an ELF".into());
    }
    let class = bytes[4];
    let le = bytes[5] == 1;
    if class == 2 {
        elf64_has_text_relocs(&bytes, le)
    } else if class == 1 {
        elf32_has_text_relocs(&bytes, le)
    } else {
        Err("unsupported ELF class".into())
    }
}

fn u16_at(b: &[u8], off: usize, le: bool) -> Option<u16> {
    let s = b.get(off..off + 2)?;
    Some(if le {
        u16::from_le_bytes([s[0], s[1]])
    } else {
        u16::from_be_bytes([s[0], s[1]])
    })
}

fn u32_at(b: &[u8], off: usize, le: bool) -> Option<u32> {
    let s = b.get(off..off + 4)?;
    Some(if le {
        u32::from_le_bytes([s[0], s[1], s[2], s[3]])
    } else {
        u32::from_be_bytes([s[0], s[1], s[2], s[3]])
    })
}

fn u64_at(b: &[u8], off: usize, le: bool) -> Option<u64> {
    let s = b.get(off..off + 8)?;
    Some(if le {
        u64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]])
    } else {
        u64::from_be_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]])
    })
}

fn elf64_has_text_relocs(b: &[u8], le: bool) -> Result<bool, String> {
    let shoff = u64_at(b, 40, le).ok_or("short ELF header")? as usize;
    let shentsize = u16_at(b, 58, le).ok_or("short ELF header")? as usize;
    let shnum = u16_at(b, 60, le).ok_or("short ELF header")? as usize;
    let shstrndx = u16_at(b, 62, le).ok_or("short ELF header")? as usize;
    section_names_contain_text_reloc(b, le, shoff, shentsize, shnum, shstrndx, true)
}

fn elf32_has_text_relocs(b: &[u8], le: bool) -> Result<bool, String> {
    let shoff = u32_at(b, 32, le).ok_or("short ELF header")? as usize;
    let shentsize = u16_at(b, 46, le).ok_or("short ELF header")? as usize;
    let shnum = u16_at(b, 48, le).ok_or("short ELF header")? as usize;
    let shstrndx = u16_at(b, 50, le).ok_or("short ELF header")? as usize;
    section_names_contain_text_reloc(b, le, shoff, shentsize, shnum, shstrndx, false)
}

fn section_names_contain_text_reloc(
    b: &[u8],
    le: bool,
    shoff: usize,
    shentsize: usize,
    shnum: usize,
    shstrndx: usize,
    elf64: bool,
) -> Result<bool, String> {
    if shentsize == 0 || shnum == 0 || shstrndx >= shnum {
        return Ok(false);
    }
    let str_hdr = shoff + shstrndx * shentsize;
    let (str_off, str_size) = if elf64 {
        (
            u64_at(b, str_hdr + 24, le).ok_or("short shdr")? as usize,
            u64_at(b, str_hdr + 32, le).ok_or("short shdr")? as usize,
        )
    } else {
        (
            u32_at(b, str_hdr + 16, le).ok_or("short shdr")? as usize,
            u32_at(b, str_hdr + 20, le).ok_or("short shdr")? as usize,
        )
    };
    let strtab = b
        .get(str_off..str_off.saturating_add(str_size))
        .ok_or("shstrtab out of range")?;
    for i in 0..shnum {
        let hdr = shoff + i * shentsize;
        let name_off = u32_at(b, hdr, le).ok_or("short shdr")? as usize;
        let name = cstr_at(strtab, name_off);
        if name == ".rela.text" || name == ".rel.text" {
            return Ok(true);
        }
    }
    Ok(false)
}

fn cstr_at(tab: &[u8], off: usize) -> String {
    if off >= tab.len() {
        return String::new();
    }
    let end = tab[off..]
        .iter()
        .position(|&c| c == 0)
        .map(|p| off + p)
        .unwrap_or(tab.len());
    String::from_utf8_lossy(&tab[off..end]).into_owned()
}

pub fn lbr_record_command(perf_data: &Path, profile_cmd: &str) -> String {
    format!(
        "perf record -e cycles:u -j any,u --no-buildid --no-buildid-cache -o {} -- {}",
        perf_data.display(),
        profile_cmd
    )
}

pub fn nl_record_command(perf_data: &Path, profile_cmd: &str) -> String {
    format!(
        "perf record -e cycles:u --no-buildid --no-buildid-cache -o {} -- {}",
        perf_data.display(),
        profile_cmd
    )
}

pub fn instrument_command(input: &Path, output: &Path, fdata: &Path) -> String {
    format!(
        "llvm-bolt {} -instrument -o {} --instrumentation-file={}",
        input.display(),
        output.display(),
        fdata.display()
    )
}

pub fn bolt_optimize_command(input: &Path, output: &Path, data: &Path, no_lbr: bool) -> String {
    let nl = if no_lbr { " -nl" } else { "" };
    format!(
        "llvm-bolt {} -o {} -data={}{nl} -reorder-blocks=ext-tsp -reorder-functions=hfsort -split-functions -split-all-cold -dyno-stats",
        input.display(),
        output.display(),
        data.display()
    )
}

/// If the profile command invokes the original ELF directly, run the instrumented
/// copy instead. Scripts that honor `QUENCH_BINARY` are left unchanged.
pub fn rewrite_profile_command_for_instrumented(
    profile_cmd: &str,
    original_binary: &Path,
    project_binary: &Path,
    instrumented: &Path,
    project_root: &Path,
) -> String {
    let mut toks = profile_cmd.split_whitespace();
    let Some(first) = toks.next() else {
        return instrumented.display().to_string();
    };
    let rest: Vec<&str> = toks.collect();
    let first_path = Path::new(first);
    let resolved = if first_path.is_absolute() {
        first_path.to_path_buf()
    } else {
        project_root.join(first_path)
    };
    let first_can = std::fs::canonicalize(&resolved).ok();
    let orig_can = std::fs::canonicalize(original_binary).ok();
    let proj_can = std::fs::canonicalize(project_binary).ok();
    let matches_elf = first_can.is_some() && (first_can == orig_can || first_can == proj_can);
    if matches_elf {
        if rest.is_empty() {
            instrumented.display().to_string()
        } else {
            format!("{} {}", instrumented.display(), rest.join(" "))
        }
    } else {
        profile_cmd.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as StdCommand;

    #[test]
    fn select_prefers_lbr_when_probe_ok() {
        let d = select_profile_mode(ProfileInputs {
            lbr_probe_ok: true,
            perf_ok: true,
            bolt_ok: true,
            bolt_rt: Some(PathBuf::from("/tmp/libbolt_rt_instr.a")),
            has_text_relocs: true,
            has_profile_cmd: true,
            force: None,
        });
        assert_eq!(d.mode, ProfileMode::Lbr);
    }

    #[test]
    fn select_instrument_when_lbr_unavailable_and_rt_present() {
        let d = select_profile_mode(ProfileInputs {
            lbr_probe_ok: false,
            perf_ok: true,
            bolt_ok: true,
            bolt_rt: Some(PathBuf::from("/tmp/libbolt_rt_instr.a")),
            has_text_relocs: true,
            has_profile_cmd: true,
            force: None,
        });
        assert_eq!(d.mode, ProfileMode::Instrument);
        assert!(d
            .warning
            .unwrap()
            .to_lowercase()
            .contains("never benchmarked"));
    }

    #[test]
    fn select_unavailable_when_rt_missing() {
        let d = select_profile_mode(ProfileInputs {
            lbr_probe_ok: false,
            perf_ok: true,
            bolt_ok: true,
            bolt_rt: None,
            has_text_relocs: true,
            has_profile_cmd: true,
            force: None,
        });
        assert_eq!(d.mode, ProfileMode::Unavailable);
        assert!(d.reason.to_lowercase().contains("libbolt_rt_instr"));
    }

    #[test]
    fn select_unavailable_without_bolt() {
        let d = select_profile_mode(ProfileInputs {
            lbr_probe_ok: true,
            perf_ok: true,
            bolt_ok: false,
            bolt_rt: Some(PathBuf::from("/tmp/x")),
            has_text_relocs: true,
            has_profile_cmd: true,
            force: None,
        });
        assert_eq!(d.mode, ProfileMode::Unavailable);
    }

    #[test]
    fn missing_libbolt_env_is_none() {
        std::env::set_var("QUENCH_BOLT_RT_INSTR", "/tmp/quench-missing-bolt-rt.a");
        assert!(find_bolt_rt_instr().is_none());
        std::env::remove_var("QUENCH_BOLT_RT_INSTR");
    }

    #[test]
    fn lbr_probe_command_is_exact() {
        let p = probe_lbr();
        assert_eq!(p.command, LBR_PROBE_COMMAND);
        assert!(LBR_PROBE_COMMAND.contains("perf record -e cycles:u -j any,u"));
        assert!(LBR_PROBE_COMMAND.contains("sleep 0.3"));
        let _ = p.ok;
    }

    #[test]
    fn instrument_command_never_includes_benchmark() {
        let cmd = instrument_command(
            Path::new("/tmp/base"),
            Path::new("/tmp/inst"),
            Path::new("/tmp/prof.fdata"),
        );
        assert!(cmd.contains("-instrument"));
        assert!(!cmd.contains("bench"));
    }

    #[test]
    fn rewrite_replaces_direct_binary_invocation() {
        let dir = std::env::temp_dir().join(format!("quench-rewrite-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let orig = dir.join("app");
        let inst = dir.join("instrumented");
        std::fs::write(&orig, b"orig").unwrap();
        std::fs::write(&inst, b"inst").unwrap();
        let rewritten =
            rewrite_profile_command_for_instrumented("./app", &orig, &orig, &inst, &dir);
        assert_eq!(rewritten, inst.display().to_string());
        let script =
            rewrite_profile_command_for_instrumented("./workload.sh", &orig, &orig, &inst, &dir);
        assert_eq!(script, "./workload.sh");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn force_mode_overrides() {
        let d = select_profile_mode(ProfileInputs {
            lbr_probe_ok: true,
            perf_ok: true,
            bolt_ok: true,
            bolt_rt: None,
            has_text_relocs: false,
            has_profile_cmd: true,
            force: Some(ProfileMode::Unavailable),
        });
        assert_eq!(d.mode, ProfileMode::Unavailable);
    }

    #[test]
    fn force_nl_is_reported() {
        let d = select_profile_mode(ProfileInputs {
            lbr_probe_ok: false,
            perf_ok: true,
            bolt_ok: true,
            bolt_rt: None,
            has_text_relocs: true,
            has_profile_cmd: true,
            force: Some(ProfileMode::Nl),
        });
        assert_eq!(d.mode, ProfileMode::Nl);
    }

    #[test]
    fn reloc_detection_on_gcc_binaries() {
        if which("gcc").is_none() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("quench-reloc-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let src = dir.join("t.c");
        std::fs::write(&src, "int main(void){return 0;}\n").unwrap();
        let no = dir.join("no-relocs");
        let yes = dir.join("relocs");
        let st1 = StdCommand::new("gcc")
            .args(["-O0", "-g", "-o"])
            .arg(&no)
            .arg(&src)
            .status()
            .unwrap();
        let st2 = StdCommand::new("gcc")
            .args(["-O0", "-g", "-Wl,--emit-relocs", "-o"])
            .arg(&yes)
            .arg(&src)
            .status()
            .unwrap();
        if st1.success() {
            let has = elf_has_text_relocs(&no).unwrap_or(false);
            assert!(!has, "default gcc binary should lack .rela.text, got {has}");
        }
        if st2.success() {
            assert!(
                elf_has_text_relocs(&yes).unwrap(),
                "emit-relocs binary must have .rela.text"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
        let bad = elf_has_text_relocs(Path::new("/tmp/definitely-not-an-elf-quench"));
        assert!(bad.is_err());
    }

    #[test]
    fn profile_info_json_includes_mode() {
        let mut info = ProfileInfo::default();
        info.mode = ProfileMode::Instrument;
        info.benchmarked_instrumented = false;
        info.benchmarked_original = true;
        let v = info.to_json();
        assert_eq!(v["profileMode"], "instrument");
        assert_eq!(v["benchmarkedInstrumented"], false);
        assert_eq!(v["benchmarkedOriginal"], true);
        assert_eq!(v["benchmarkUsedUninstrumentedBinaries"], true);
    }
}
