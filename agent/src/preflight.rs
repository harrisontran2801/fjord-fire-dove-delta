use crate::config::{load_config, validate_for_optimize, ProjectKind, QuenchConfig};
use crate::doctor::{run_doctor, tool_available, CheckStatus, DoctorReport};
use crate::util::which;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IssueSeverity {
    Blocking,
    Warning,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreflightIssue {
    pub id: String,
    pub severity: IssueSeverity,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandCheck {
    pub role: String,
    pub command: Option<String>,
    pub required: bool,
    pub available: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreflightReport {
    pub ok: bool,
    pub config_path: String,
    pub project: Option<String>,
    pub kind: Option<String>,
    pub issues: Vec<PreflightIssue>,
    pub commands: Vec<CommandCheck>,
    pub min_improvement_percent: Option<f64>,
    pub max_regression_percent: Option<f64>,
    pub doctor: DoctorReport,
}

impl PreflightReport {
    pub fn blocking_summary(&self) -> String {
        let lines: Vec<&str> = self
            .issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Blocking)
            .map(|i| i.detail.as_str())
            .collect();
        if lines.is_empty() {
            "preflight failed".into()
        } else {
            format!("preflight failed: {}", lines.join("; "))
        }
    }
}

fn issue(id: &str, severity: IssueSeverity, detail: impl Into<String>) -> PreflightIssue {
    PreflightIssue {
        id: id.into(),
        severity,
        detail: detail.into(),
    }
}

fn first_token(command: &str) -> Option<&str> {
    command.split_whitespace().find(|tok| {
        if tok.starts_with('-') {
            return false;
        }
        if tok.contains('=') && !tok.starts_with('/') && !tok.starts_with('.') {
            return false;
        }
        true
    })
}

fn looks_like_path(token: &str) -> bool {
    token.starts_with('.') || token.starts_with('/') || token.contains('/')
}

fn relative_escapes_root(token: &str) -> bool {
    let mut depth = 0i32;
    for c in Path::new(token).components() {
        match c {
            std::path::Component::ParentDir => {
                depth -= 1;
                if depth < 0 {
                    return true;
                }
            }
            std::path::Component::Normal(_) => depth += 1,
            _ => {}
        }
    }
    false
}

fn check_command(
    role: &str,
    command: Option<&str>,
    required: bool,
    root: &Path,
    issues: &mut Vec<PreflightIssue>,
) -> CommandCheck {
    let Some(cmd) = command.map(str::trim).filter(|s| !s.is_empty()) else {
        let detail = if required {
            format!("Missing `{role}` command in quench.yaml. Native optimize will not invent one.")
        } else {
            format!("No `{role}` command; optional workload/profile step will be skipped.")
        };
        if required {
            issues.push(issue(role, IssueSeverity::Blocking, detail.clone()));
        } else {
            issues.push(issue(role, IssueSeverity::Warning, detail.clone()));
        }
        return CommandCheck {
            role: role.into(),
            command: None,
            required,
            available: !required,
            detail,
        };
    };

    let Some(token) = first_token(cmd) else {
        let detail = format!("`{role}` command is empty after parsing: {cmd}");
        issues.push(issue(role, IssueSeverity::Blocking, detail.clone()));
        return CommandCheck {
            role: role.into(),
            command: Some(cmd.into()),
            required,
            available: false,
            detail,
        };
    };

    if looks_like_path(token) {
        let path = if Path::new(token).is_absolute() {
            PathBuf::from(token)
        } else {
            root.join(token)
        };
        if !Path::new(token).is_absolute() && relative_escapes_root(token) {
            let detail = format!("`{role}` path `{token}` is outside the project root");
            issues.push(issue(role, IssueSeverity::Blocking, detail.clone()));
            return CommandCheck {
                role: role.into(),
                command: Some(cmd.into()),
                required,
                available: false,
                detail,
            };
        }
        if path.is_file() {
            return CommandCheck {
                role: role.into(),
                command: Some(cmd.into()),
                required,
                available: true,
                detail: format!("found {}", path.display()),
            };
        }
        let detail = format!("`{role}` script not found: {}", path.display());
        if required {
            issues.push(issue(role, IssueSeverity::Blocking, detail.clone()));
        } else {
            issues.push(issue(role, IssueSeverity::Warning, detail.clone()));
        }
        return CommandCheck {
            role: role.into(),
            command: Some(cmd.into()),
            required,
            available: false,
            detail,
        };
    }

    match which(token) {
        Some(found) => {
            if token == "cargo" && which("rustc").is_none() {
                let detail = "cargo is present but rustc is not on PATH";
                issues.push(issue("rustc", IssueSeverity::Blocking, detail));
                return CommandCheck {
                    role: role.into(),
                    command: Some(cmd.into()),
                    required,
                    available: false,
                    detail: detail.into(),
                };
            }
            CommandCheck {
                role: role.into(),
                command: Some(cmd.into()),
                required,
                available: true,
                detail: format!("found {found}"),
            }
        }
        None => {
            let detail = format!("`{role}` tool `{token}` is not on PATH");
            if required {
                issues.push(issue(token, IssueSeverity::Blocking, detail.clone()));
            } else {
                issues.push(issue(token, IssueSeverity::Warning, detail.clone()));
            }
            CommandCheck {
                role: role.into(),
                command: Some(cmd.into()),
                required,
                available: false,
                detail,
            }
        }
    }
}

pub fn evaluate(cfg: &QuenchConfig, doctor: &DoctorReport) -> PreflightReport {
    let mut issues = Vec::new();
    let mut commands = Vec::new();

    if !doctor.supported_platform {
        issues.push(issue(
            "platform",
            IssueSeverity::Blocking,
            "Native optimize only supports Linux x86_64. Windows and macOS are not supported.",
        ));
    }

    if let Err(e) = validate_for_optimize(cfg) {
        issues.push(issue("config", IssueSeverity::Blocking, e));
    }

    let root = &cfg.project_root;
    let elf = matches!(cfg.kind, ProjectKind::Elf);
    commands.push(check_command(
        "build",
        cfg.build.as_deref(),
        elf,
        root,
        &mut issues,
    ));
    commands.push(check_command(
        "test",
        cfg.test.as_deref(),
        elf,
        root,
        &mut issues,
    ));
    commands.push(check_command(
        "benchmark",
        cfg.benchmark.as_deref(),
        elf,
        root,
        &mut issues,
    ));
    commands.push(check_command(
        "profile",
        cfg.profile.as_deref(),
        false,
        root,
        &mut issues,
    ));

    if elf && !tool_available(doctor, "strip") {
        issues.push(issue(
            "strip",
            IssueSeverity::Warning,
            "strip is Unavailable; no symbol-stripping transform will be applied. Metrics will not be invented.",
        ));
    }
    if !tool_available(doctor, "perf") {
        issues.push(issue(
            "perf",
            IssueSeverity::Warning,
            "perf is Unavailable; no LBR profile will be collected.",
        ));
    } else {
        issues.push(issue(
            "lbr",
            IssueSeverity::Warning,
            "Optimize probes LBR with `perf record -e cycles:u -j any,u -- sleep 0.3`. If that fails, BOLT instrumentation is used when libbolt_rt_instr.a is present. Instrumentation is not production sampling and is never benchmarked.",
        ));
    }
    if !tool_available(doctor, "llvm-bolt") {
        issues.push(issue(
            "llvm-bolt",
            IssueSeverity::Warning,
            "llvm-bolt is Unavailable; no layout rewrite will be performed.",
        ));
    }

    let ok = issues.iter().all(|i| i.severity != IssueSeverity::Blocking);
    PreflightReport {
        ok,
        config_path: cfg.source_path.display().to_string(),
        project: Some(cfg.project.clone()),
        kind: Some(cfg.kind.as_str().into()),
        issues,
        commands,
        min_improvement_percent: Some(cfg.min_improvement_percent),
        max_regression_percent: Some(cfg.max_regression_percent),
        doctor: doctor.clone(),
    }
}

pub fn run_preflight(config_path: &Path) -> PreflightReport {
    let doctor = run_doctor();
    if !config_path.exists() {
        return PreflightReport {
            ok: false,
            config_path: config_path.display().to_string(),
            project: None,
            kind: None,
            issues: vec![issue(
                "config",
                IssueSeverity::Blocking,
                format!("config not found: {}", config_path.display()),
            )],
            commands: vec![],
            min_improvement_percent: None,
            max_regression_percent: None,
            doctor,
        };
    }
    match load_config(config_path) {
        Ok(cfg) => evaluate(&cfg, &doctor),
        Err(e) => PreflightReport {
            ok: false,
            config_path: config_path.display().to_string(),
            project: None,
            kind: None,
            issues: vec![issue("config", IssueSeverity::Blocking, e)],
            commands: vec![],
            min_improvement_percent: None,
            max_regression_percent: None,
            doctor,
        },
    }
}

pub fn preflight_text(report: &PreflightReport) -> String {
    let mut lines = vec![
        "quench-agent preflight".into(),
        format!("config     {}", report.config_path),
        format!("project    {}", report.project.as_deref().unwrap_or("-")),
        format!("kind       {}", report.kind.as_deref().unwrap_or("-")),
        format!(
            "result     {}",
            if report.ok {
                "ready"
            } else {
                "not ready — blocking issues"
            }
        ),
    ];
    if let (Some(min), Some(max)) = (
        report.min_improvement_percent,
        report.max_regression_percent,
    ) {
        lines.push(format!(
            "gates      min_improvement_percent={min} max_regression_percent={max}"
        ));
    }
    lines.push("commands:".into());
    if report.commands.is_empty() {
        lines.push("  (none — config did not load)".into());
    }
    for c in &report.commands {
        let req = if c.required { "required" } else { "optional" };
        let avail = if c.available { "OK" } else { "Unavailable" };
        lines.push(format!(
            "  {} [{}] {}: {}",
            c.role,
            req,
            avail,
            c.command.as_deref().unwrap_or("-")
        ));
        lines.push(format!("           {}", c.detail));
    }
    lines.push("issues:".into());
    if report.issues.is_empty() {
        lines.push("  (none)".into());
    }
    for i in &report.issues {
        let sev = match i.severity {
            IssueSeverity::Blocking => "BLOCKING",
            IssueSeverity::Warning => "WARN",
        };
        lines.push(format!("  [{sev}] {}: {}", i.id, i.detail));
    }
    lines.push("Host tools (missing tools are Unavailable; results are not invented):".into());
    for c in &report.doctor.checks {
        let status = match c.status {
            CheckStatus::Ok => "OK",
            CheckStatus::Warn => "WARN",
            CheckStatus::Unavailable => "Unavailable",
        };
        lines.push(format!("  {:<12} {status}  {}", c.id, c.detail));
    }
    if !report.ok {
        lines.push(report.blocking_summary());
    }
    lines.join("\n") + "\n"
}

pub fn preflight_json(report: &PreflightReport) -> serde_json::Value {
    serde_json::to_value(report).unwrap_or(serde_json::json!({"ok": false}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::now_ms;
    use std::fs;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn write_exec(path: &Path, body: &str) {
        fs::write(path, body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    fn temp_project(
        name: &str,
        yaml: &str,
        with_scripts: bool,
    ) -> (PathBuf, std::sync::MutexGuard<'static, ()>) {
        let guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let root = std::env::temp_dir().join(format!(
            "quench-pf-{}-{}-{}",
            name,
            std::process::id(),
            now_ms()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("quench.yaml"), yaml).unwrap();
        if with_scripts {
            fs::write(root.join("main.c"), "int main(void){return 0;}\n").unwrap();
            write_exec(root.join("test.sh").as_path(), "#!/bin/sh\nexit 0\n");
            write_exec(
                root.join("bench.sh").as_path(),
                "#!/bin/sh\necho elapsed_ms=1\n",
            );
            write_exec(root.join("workload.sh").as_path(), "#!/bin/sh\nexit 0\n");
        }
        (root, guard)
    }

    #[test]
    fn missing_config_is_blocking() {
        let report = run_preflight(Path::new("/tmp/does-not-exist-quench.yaml"));
        assert!(!report.ok);
        assert!(report
            .issues
            .iter()
            .any(|i| i.id == "config" && i.severity == IssueSeverity::Blocking));
        assert!(report.blocking_summary().contains("not found"));
    }

    #[test]
    fn missing_benchmark_field_is_blocking() {
        let yaml = r#"
project: incomplete
kind: elf
binary: ./app
build: gcc -o app main.c
test: ./test.sh
"#;
        let (root, _g) = temp_project("incomplete", yaml, true);
        let report = run_preflight(&root.join("quench.yaml"));
        assert!(!report.ok);
        let joined = report.blocking_summary();
        assert!(
            joined.contains("benchmark") || joined.contains("Missing input"),
            "{joined}"
        );
    }

    #[test]
    fn missing_benchmark_script_is_blocking() {
        let yaml = r#"
project: missing-script
kind: elf
binary: ./app
build: gcc -o app main.c
test: ./test.sh
benchmark: ./nope.sh
profile: ./workload.sh
max_regression_percent: 2
min_improvement_percent: 1
"#;
        let (root, _g) = temp_project("missing-script", yaml, true);
        let report = run_preflight(&root.join("quench.yaml"));
        assert!(!report.ok);
        assert!(report
            .issues
            .iter()
            .any(|i| i.id == "benchmark" && i.severity == IssueSeverity::Blocking));
        assert!(report.blocking_summary().contains("nope.sh"));
    }

    #[test]
    fn customer_config_with_scripts_is_ready() {
        let yaml = r#"
project: customer-elf
kind: elf
binary: ./app
build: gcc -O0 -g -o app main.c
test: ./test.sh
benchmark: ./bench.sh
profile: ./workload.sh
max_regression_percent: 2
min_improvement_percent: 1
"#;
        let (root, _g) = temp_project("customer", yaml, true);
        let report = run_preflight(&root.join("quench.yaml"));
        assert!(report.ok, "{}", report.blocking_summary());
        assert_eq!(report.project.as_deref(), Some("customer-elf"));
        assert_eq!(report.min_improvement_percent, Some(1.0));
        assert_eq!(report.max_regression_percent, Some(2.0));
        assert!(report.commands.iter().all(|c| {
            if c.required {
                c.available
            } else {
                true
            }
        }));
    }

    #[test]
    fn invalid_yaml_is_blocking() {
        let (root, _g) = temp_project("badyaml", "this is not: valid: yaml: :::\n", false);
        let report = run_preflight(&root.join("quench.yaml"));
        assert!(!report.ok);
        assert!(report.issues.iter().any(|i| i.id == "config"));
    }

    #[test]
    fn relative_escape_is_blocking() {
        let yaml = r#"
project: escape
kind: elf
binary: ./app
build: gcc -o app main.c
test: ./test.sh
benchmark: ../../etc/passwd
"#;
        let (root, _g) = temp_project("escape", yaml, true);
        let report = run_preflight(&root.join("quench.yaml"));
        assert!(!report.ok);
        let joined = report.blocking_summary();
        assert!(
            joined.contains("outside") || joined.contains("not found"),
            "{joined}"
        );
    }

    #[test]
    fn bundled_match_engine_preflight_is_ready() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../samples/match-engine/quench.yaml");
        let report = run_preflight(&path);
        assert!(report.ok, "{}", preflight_text(&report));
        assert_eq!(report.project.as_deref(), Some("match-engine"));
        assert!(report
            .commands
            .iter()
            .find(|c| c.role == "build")
            .is_some_and(|c| c.available));
        assert!(report
            .commands
            .iter()
            .find(|c| c.role == "profile")
            .is_some_and(|c| c.command.as_deref() == Some("./scripts/workload.sh")));
    }
}
