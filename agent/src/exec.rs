use crate::util::{first_line, sha256_file, truncate_utf8, which};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRecord {
    pub command: String,
    pub cwd: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_version: Option<String>,
    pub timed_out: bool,
}

pub fn tool_version(bin: &str) -> Option<String> {
    let path = which(bin)?;
    let out = Command::new(&path).arg("--version").output().ok()?;
    let text = if out.stdout.is_empty() {
        String::from_utf8_lossy(&out.stderr).into_owned()
    } else {
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let line = first_line(&text);
    if line.is_empty() {
        Some(path)
    } else {
        Some(line)
    }
}

fn command_bin(command: &str) -> Option<String> {
    for tok in command.split_whitespace() {
        if tok.contains('=')
            && !tok.starts_with('/')
            && !tok.starts_with('.')
            && !tok.starts_with('-')
        {
            continue;
        }
        if tok.starts_with('-') {
            continue;
        }
        return Some(tok.to_string());
    }
    None
}

fn version_for_command(command: &str) -> Option<String> {
    let bin = command_bin(command)?;
    let name = Path::new(&bin).file_name()?.to_str()?;
    tool_version(name)
}

fn read_capped(mut pipe: impl Read, cap: usize) -> String {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 8192];
    loop {
        match pipe.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => {
                if buf.len() < cap {
                    let take = n.min(cap - buf.len());
                    buf.extend_from_slice(&tmp[..take]);
                }
            }
            Err(_) => break,
        }
    }
    String::from_utf8_lossy(&buf).into_owned()
}

/// Flags for a future container workload. Not applied in this MVP because
/// Docker/OCI optimize is inspect/recommendation only and no container is started.
pub fn docker_security_args(timeout_secs: u64, memory: &str, cpus: &str) -> Vec<String> {
    vec![
        "run".into(),
        "--rm".into(),
        "--network".into(),
        "none".into(),
        "--read-only".into(),
        "--cap-drop".into(),
        "ALL".into(),
        "--security-opt".into(),
        "no-new-privileges".into(),
        "--user".into(),
        "65534:65534".into(),
        "--memory".into(),
        memory.into(),
        "--cpus".into(),
        cpus.into(),
        "--pids-limit".into(),
        "128".into(),
        "--tmpfs".into(),
        "/tmp:rw,noexec,nosuid,size=64m".into(),
        "--stop-timeout".into(),
        timeout_secs.to_string(),
    ]
}

pub fn run_command(
    command: &str,
    cwd: &Path,
    timeout: Duration,
    extra_env: &[(&str, &str)],
    artifact: Option<&Path>,
) -> CommandRecord {
    let started = Instant::now();
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(command)
        .current_dir(cwd)
        .env("QUENCH_AGENT", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    if command.contains("docker") && command.contains("run") {
        if command.contains("docker.sock") {
            return CommandRecord {
                command: command.into(),
                cwd: cwd.display().to_string(),
                exit_code: 2,
                stdout: String::new(),
                stderr: "refusing to mount the Docker socket into a workload".into(),
                duration_ms: started.elapsed().as_millis() as u64,
                artifact_path: artifact.map(|p| p.display().to_string()),
                sha256: None,
                tool_version: version_for_command(command),
                timed_out: false,
            };
        }
        if command.contains("--privileged") {
            return CommandRecord {
                command: command.into(),
                cwd: cwd.display().to_string(),
                exit_code: 2,
                stdout: String::new(),
                stderr: "refusing to run a privileged container".into(),
                duration_ms: started.elapsed().as_millis() as u64,
                artifact_path: artifact.map(|p| p.display().to_string()),
                sha256: None,
                tool_version: version_for_command(command),
                timed_out: false,
            };
        }
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return CommandRecord {
                command: command.into(),
                cwd: cwd.display().to_string(),
                exit_code: 127,
                stdout: String::new(),
                stderr: format!("failed to spawn: {e}"),
                duration_ms: started.elapsed().as_millis() as u64,
                artifact_path: artifact.map(|p| p.display().to_string()),
                sha256: None,
                tool_version: version_for_command(command),
                timed_out: false,
            };
        }
    };
    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();
    let cap = 256 * 1024;
    let stdout_handle = std::thread::spawn(move || {
        stdout_pipe
            .as_mut()
            .map(|p| read_capped(p, cap))
            .unwrap_or_default()
    });
    let stderr_handle = std::thread::spawn(move || {
        stderr_pipe
            .as_mut()
            .map(|p| read_capped(p, cap))
            .unwrap_or_default()
    });

    let deadline = Instant::now() + timeout;
    let mut timed_out = false;
    let mut exit_code;
    loop {
        match child.try_wait() {
            Ok(Some(st)) => {
                exit_code = st.code().unwrap_or(1);
                break;
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    timed_out = true;
                    let _ = child.kill();
                    let _ = child.wait();
                    exit_code = 124;
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => {
                return CommandRecord {
                    command: command.into(),
                    cwd: cwd.display().to_string(),
                    exit_code: 1,
                    stdout: String::new(),
                    stderr: format!("wait failed: {e}"),
                    duration_ms: started.elapsed().as_millis() as u64,
                    artifact_path: artifact.map(|p| p.display().to_string()),
                    sha256: None,
                    tool_version: version_for_command(command),
                    timed_out: false,
                };
            }
        }
    }

    let stdout = truncate_utf8(&stdout_handle.join().unwrap_or_default(), 200_000);
    let mut stderr = truncate_utf8(&stderr_handle.join().unwrap_or_default(), 200_000);
    if timed_out {
        stderr = format!("timed out after {}ms\n{stderr}", timeout.as_millis());
    }
    let sha = artifact.and_then(|p| sha256_file(p).ok());
    CommandRecord {
        command: command.into(),
        cwd: cwd.display().to_string(),
        exit_code,
        stdout,
        stderr,
        duration_ms: started.elapsed().as_millis() as u64,
        artifact_path: artifact.map(|p| p.display().to_string()),
        sha256: sha,
        tool_version: version_for_command(command),
        timed_out,
    }
}

pub fn write_log_line(path: &Path, line: &str) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{line}");
    }
}

pub fn resolve_in_root(root: &Path, rel: &str) -> PathBuf {
    let p = Path::new(rel);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        root.join(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docker_security_args_are_restrictive() {
        let args = docker_security_args(30, "512m", "1");
        let joined = args.join(" ");
        assert!(joined.contains("--network none"));
        assert!(joined.contains("--cap-drop ALL"));
        assert!(joined.contains("no-new-privileges"));
        assert!(!joined.contains("docker.sock"));
        assert!(!joined.contains("--privileged"));
    }

    #[test]
    fn records_tool_version_for_known_binaries() {
        let rec = run_command(
            "gcc --version",
            Path::new("."),
            Duration::from_secs(10),
            &[],
            None,
        );
        assert_eq!(rec.exit_code, 0);
        assert!(
            rec.tool_version.is_some(),
            "expected gcc version on CommandRecord"
        );
        assert!(rec
            .tool_version
            .as_deref()
            .unwrap()
            .to_lowercase()
            .contains("gcc"));
    }

    #[test]
    fn refuses_docker_socket_mount() {
        let rec = run_command(
            "docker run -v /var/run/docker.sock:/var/run/docker.sock alpine",
            Path::new("."),
            Duration::from_secs(5),
            &[],
            None,
        );
        assert_eq!(rec.exit_code, 2);
        assert!(rec.stderr.contains("Docker socket"));
    }
}
