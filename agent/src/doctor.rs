use crate::util::{first_line, which};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Ok,
    Warn,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Check {
    pub id: String,
    pub name: String,
    pub status: CheckStatus,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub ok: bool,
    pub supported_platform: bool,
    pub platform: String,
    pub arch: String,
    pub work_dir: String,
    pub checks: Vec<Check>,
}

fn version_of(bin: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(bin).args(args).output().ok()?;
    let text = if out.stdout.is_empty() {
        String::from_utf8_lossy(&out.stderr).into_owned()
    } else {
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let line = first_line(&text);
    if line.is_empty() {
        None
    } else {
        Some(line)
    }
}

fn tool_check(id: &str, name: &str, bins: &[&str], ver_args: &[&str]) -> Check {
    for bin in bins {
        if let Some(path) = which(bin) {
            return Check {
                id: id.into(),
                name: name.into(),
                status: CheckStatus::Ok,
                detail: format!("found {path}"),
                version: version_of(bin, ver_args),
                path: Some(path),
            };
        }
    }
    Check {
        id: id.into(),
        name: name.into(),
        status: CheckStatus::Unavailable,
        detail: format!("{} not found on PATH", bins.join(" / ")),
        version: None,
        path: None,
    }
}

fn platform_check() -> (bool, Check) {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let supported = os == "linux" && arch == "x86_64";
    let detail = format!("{os}/{arch}");
    (
        supported,
        Check {
            id: "platform".into(),
            name: "Linux x86_64".into(),
            status: if supported {
                CheckStatus::Ok
            } else {
                CheckStatus::Unavailable
            },
            detail: if supported {
                format!("supported ({detail})")
            } else {
                format!("this agent only supports Linux x86_64; current is {detail}")
            },
            version: Some(detail),
            path: None,
        },
    )
}

fn exec_check(work_dir: &Path) -> Check {
    let probe = work_dir.join(".quench-exec-probe.sh");
    let write = std::fs::create_dir_all(work_dir)
        .and_then(|_| std::fs::write(&probe, b"#!/bin/sh\nexit 0\n"));
    #[cfg(unix)]
    let mode_ok = {
        use std::os::unix::fs::PermissionsExt;
        match write {
            Ok(()) => {
                std::fs::set_permissions(&probe, std::fs::Permissions::from_mode(0o755)).is_ok()
            }
            Err(_) => false,
        }
    };
    #[cfg(not(unix))]
    let mode_ok = write.is_ok();
    let ran = mode_ok
        && Command::new(&probe)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    let _ = std::fs::remove_file(&probe);
    if ran {
        Check {
            id: "exec".into(),
            name: "Execute permission".into(),
            status: CheckStatus::Ok,
            detail: format!("can create and run files in {}", work_dir.display()),
            version: None,
            path: None,
        }
    } else {
        Check {
            id: "exec".into(),
            name: "Execute permission".into(),
            status: CheckStatus::Unavailable,
            detail: format!("cannot execute files in {}", work_dir.display()),
            version: None,
            path: None,
        }
    }
}

fn disk_check(work_dir: &Path) -> Check {
    let _ = std::fs::create_dir_all(work_dir);
    let output = Command::new("df")
        .args(["-Pk", &work_dir.to_string_lossy()])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            let avail_kb = text
                .lines()
                .nth(1)
                .and_then(|line| line.split_whitespace().nth(3))
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            let avail = avail_kb.saturating_mul(1024);
            let status = if avail >= 256 * 1024 * 1024 {
                CheckStatus::Ok
            } else if avail > 0 {
                CheckStatus::Warn
            } else {
                CheckStatus::Unavailable
            };
            Check {
                id: "disk".into(),
                name: "Working directory space".into(),
                status,
                detail: format!("{} bytes available in {}", avail, work_dir.display()),
                version: Some(avail.to_string()),
                path: Some(work_dir.display().to_string()),
            }
        }
        _ => Check {
            id: "disk".into(),
            name: "Working directory space".into(),
            status: CheckStatus::Warn,
            detail: "could not query disk space".into(),
            version: None,
            path: Some(work_dir.display().to_string()),
        },
    }
}

pub fn work_dir() -> PathBuf {
    if let Ok(p) = std::env::var("QUENCH_HOME") {
        return PathBuf::from(p);
    }
    if let Ok(p) = std::env::var("HOME") {
        return PathBuf::from(p).join(".quench");
    }
    PathBuf::from("/tmp/quench")
}

pub fn run_doctor() -> DoctorReport {
    let work = work_dir();
    let (supported, platform) = platform_check();
    let clang = tool_check(
        "clang",
        "LLVM/Clang",
        &["clang", "clang-19", "clang-18"],
        &["--version"],
    );
    let bolt = tool_check("llvm-bolt", "llvm-bolt", &["llvm-bolt"], &["--version"]);
    let perf = tool_check("perf", "perf", &["perf"], &["--version"]);
    let docker = tool_check(
        "docker",
        "Docker or containerd",
        &["docker", "containerd", "nerdctl"],
        &["--version"],
    );
    let strip = tool_check("strip", "strip", &["strip"], &["--version"]);
    let exec = exec_check(&work);
    let disk = disk_check(&work);
    let checks = vec![platform, clang, bolt, perf, docker, strip, exec, disk];
    let blocking_fail = checks.iter().any(|c| {
        matches!(c.id.as_str(), "platform" | "exec") && c.status == CheckStatus::Unavailable
    });
    DoctorReport {
        ok: supported && !blocking_fail,
        supported_platform: supported,
        platform: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        work_dir: work.display().to_string(),
        checks,
    }
}

pub fn doctor_text(report: &DoctorReport) -> String {
    let mut lines = Vec::new();
    lines.push("quench-agent doctor".into());
    lines.push(format!("platform    {}/{}", report.platform, report.arch));
    lines.push(format!("work dir    {}", report.work_dir));
    for c in &report.checks {
        let status = match c.status {
            CheckStatus::Ok => "OK",
            CheckStatus::Warn => "WARN",
            CheckStatus::Unavailable => "Unavailable",
        };
        let ver = c.version.as_deref().unwrap_or("-");
        lines.push(format!(
            "{:<12} {:<12} {}",
            c.name,
            status,
            if c.detail.is_empty() { ver } else { &c.detail }
        ));
    }
    if !report.supported_platform {
        lines.push("Native optimize is disabled: Linux x86_64 required.".into());
    }
    lines.join("\n") + "\n"
}

pub fn doctor_json(report: &DoctorReport) -> serde_json::Value {
    json!(report)
}

pub fn check_status<'a>(report: &'a DoctorReport, id: &str) -> Option<&'a Check> {
    report.checks.iter().find(|c| c.id == id)
}

pub fn tool_available(report: &DoctorReport, id: &str) -> bool {
    check_status(report, id).is_some_and(|c| c.status == CheckStatus::Ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_marks_missing_bolt_unavailable() {
        let report = run_doctor();
        let bolt = check_status(&report, "llvm-bolt").expect("bolt check");
        if which("llvm-bolt").is_none() {
            assert_eq!(bolt.status, CheckStatus::Unavailable);
            assert!(
                bolt.detail.to_lowercase().contains("not found")
                    || bolt.detail.contains("Unavailable")
            );
        }
    }

    #[test]
    fn doctor_platform_is_linux_x86_64_here() {
        let report = run_doctor();
        if std::env::consts::OS == "linux" && std::env::consts::ARCH == "x86_64" {
            assert!(report.supported_platform);
            assert_eq!(
                check_status(&report, "platform").unwrap().status,
                CheckStatus::Ok
            );
        }
    }
}
