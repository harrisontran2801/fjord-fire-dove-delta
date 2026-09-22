use crate::config::{validate_for_optimize, ProjectKind, QuenchConfig};
use crate::doctor::{run_doctor, tool_available, DoctorReport};
use crate::exec::{resolve_in_root, run_command, tool_version, write_log_line, CommandRecord};
use crate::inspect::{inspect_path, InspectOutcome};
use crate::util::{copy_file, file_size, new_run_id, now_ms, sha256_file, which, write_json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchStats {
    pub samples_ms: Vec<f64>,
    pub median_ms: f64,
    pub p95_ms: f64,
    pub repetitions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Complete,
    VerificationFailed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transform {
    pub tool: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineRun {
    pub run_id: String,
    pub status: RunStatus,
    pub created_at: u64,
    pub project: String,
    pub kind: String,
    pub input_path: String,
    pub config_path: String,
    pub logs: Vec<String>,
    pub commands: Vec<CommandRecord>,
    pub transforms_applied: Vec<Transform>,
    pub transforms_failed: Vec<Transform>,
    pub report: Value,
}

fn log_run(run_dir: &Path, run: &mut PipelineRun, stage: &str, line: &str) {
    let msg = format!("[{stage}] {line}");
    run.logs.push(msg.clone());
    write_log_line(&run_dir.join("pipeline.log"), &msg);
}

fn parse_elapsed_ms(stdout: &str, duration_ms: u64) -> f64 {
    for line in stdout.lines().rev() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("QUENCH_BENCH ") {
            if let Ok(v) = serde_json::from_str::<Value>(rest) {
                if let Some(n) = v.get("elapsed_ms").and_then(|x| x.as_f64()) {
                    return n;
                }
                if let Some(n) = v.get("median_ms").and_then(|x| x.as_f64()) {
                    return n;
                }
            }
        }
        if let Some(idx) = t.find("elapsed_ms=") {
            let num = &t[idx + "elapsed_ms=".len()..];
            let end = num
                .find(|c: char| {
                    !(c.is_ascii_digit()
                        || c == '.'
                        || c == 'e'
                        || c == 'E'
                        || c == '-'
                        || c == '+')
                })
                .unwrap_or(num.len());
            if let Ok(n) = num[..end].parse::<f64>() {
                return n;
            }
        }
        if t.starts_with('{') {
            if let Ok(v) = serde_json::from_str::<Value>(t) {
                if let Some(n) = v.get("elapsed_ms").and_then(|x| x.as_f64()) {
                    return n;
                }
            }
        }
    }
    duration_ms as f64
}

fn median(sorted: &[f64]) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let n = sorted.len();
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    }
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let idx = (p / 100.0) * (sorted.len() as f64 - 1.0);
    let lo = idx.floor() as usize;
    let hi = idx.ceil() as usize;
    let w = idx - lo as f64;
    sorted[lo] * (1.0 - w) + sorted[hi] * w
}

pub fn stats_from_samples(mut samples: Vec<f64>) -> BenchStats {
    samples.retain(|s| s.is_finite() && *s >= 0.0);
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let repetitions = samples.len();
    BenchStats {
        median_ms: median(&samples),
        p95_ms: percentile(&samples, 95.0),
        repetitions,
        samples_ms: samples,
    }
}

pub fn regression_pct(baseline: f64, candidate: f64) -> f64 {
    if baseline <= 0.0 {
        return 0.0;
    }
    ((candidate - baseline) / baseline) * 100.0
}

pub fn should_discard_regression(
    baseline: &BenchStats,
    candidate: &BenchStats,
    max_regression_percent: f64,
) -> Option<String> {
    let med = regression_pct(baseline.median_ms, candidate.median_ms);
    let p95 = regression_pct(baseline.p95_ms, candidate.p95_ms);
    if med > max_regression_percent {
        return Some(format!(
            "median regression {med:.3}% exceeds max_regression_percent {max_regression_percent}"
        ));
    }
    if p95 > max_regression_percent {
        return Some(format!(
            "p95 regression {p95:.3}% exceeds max_regression_percent {max_regression_percent}"
        ));
    }
    None
}

pub fn median_improvement_pct(baseline: f64, candidate: f64) -> f64 {
    if baseline <= 0.0 {
        return 0.0;
    }
    ((baseline - candidate) / baseline) * 100.0
}

/// Keep a candidate only when it does not regress beyond `max_regression_percent`
/// and median improvement meets `min_improvement_percent`. Both settings are
/// applied; a parsed-but-unused threshold is not allowed.
pub fn should_discard_candidate(
    baseline: &BenchStats,
    candidate: &BenchStats,
    max_regression_percent: f64,
    min_improvement_percent: f64,
) -> Option<String> {
    if let Some(reason) = should_discard_regression(baseline, candidate, max_regression_percent) {
        return Some(reason);
    }
    let med = median_improvement_pct(baseline.median_ms, candidate.median_ms);
    if med < min_improvement_percent {
        return Some(format!(
            "median improvement {med:.3}% is below min_improvement_percent {min_improvement_percent}"
        ));
    }
    None
}

fn forced_unavailable(id: &str) -> bool {
    std::env::var("QUENCH_FORCE_UNAVAILABLE")
        .ok()
        .map(|v| v.split(',').map(str::trim).any(|s| s == id))
        .unwrap_or(false)
}

fn can_use(doctor: &DoctorReport, id: &str) -> bool {
    if forced_unavailable(id) {
        return false;
    }
    if id == "strip" {
        return which("strip").is_some();
    }
    tool_available(doctor, id)
}

fn timeout() -> Duration {
    Duration::from_secs(
        std::env::var("QUENCH_CMD_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(180),
    )
}

fn record(run: &mut PipelineRun, rec: CommandRecord) -> &CommandRecord {
    run.commands.push(rec);
    run.commands.last().unwrap()
}

fn run_bench(
    cfg: &QuenchConfig,
    run: &mut PipelineRun,
    run_dir: &Path,
    binary: &Path,
    times: usize,
    stage: &str,
) -> Result<BenchStats, String> {
    let cmd = cfg.benchmark.clone().unwrap();
    let mut samples = Vec::new();
    for i in 1..=times {
        log_run(
            run_dir,
            run,
            "benchmark",
            &format!("{stage} repetition {i}/{times}"),
        );
        let rec = run_command(
            &cmd,
            &cfg.project_root,
            timeout(),
            &[
                ("QUENCH_BINARY", &binary.to_string_lossy()),
                ("QUENCH_STAGE", stage),
            ],
            Some(binary),
        );
        if rec.exit_code != 0 {
            let err = format!("benchmark command failed (exit {})", rec.exit_code);
            record(run, rec);
            return Err(err);
        }
        let ms = parse_elapsed_ms(&rec.stdout, rec.duration_ms);
        samples.push(ms);
        log_run(
            run_dir,
            run,
            "benchmark",
            &format!("{stage}[{i}] elapsed_ms={ms:.4} exit={}", rec.exit_code),
        );
        record(run, rec);
    }
    Ok(stats_from_samples(samples))
}

fn artifact_meta(path: &Path) -> Value {
    json!({
        "path": path.display().to_string(),
        "sha256": sha256_file(path).ok(),
        "sizeBytes": file_size(path).ok(),
    })
}

fn tool_versions(doctor: &DoctorReport) -> Value {
    let mut map = serde_json::Map::new();
    for c in &doctor.checks {
        map.insert(
            c.id.clone(),
            json!({
                "status": c.status,
                "version": c.version,
                "path": c.path,
                "detail": c.detail,
            }),
        );
    }
    if let Some(v) = tool_version("rustc") {
        map.insert("rustc".into(), json!({ "status": "ok", "version": v }));
    }
    if let Some(v) = tool_version("cargo") {
        map.insert("cargo".into(), json!({ "status": "ok", "version": v }));
    }
    Value::Object(map)
}

fn finalize_report(run: &mut PipelineRun, cfg: &QuenchConfig, extra: Value) {
    let mut report = extra;
    if let Some(obj) = report.as_object_mut() {
        obj.insert("runId".into(), json!(run.run_id));
        obj.insert("project".into(), json!(run.project));
        obj.insert("kind".into(), json!(run.kind));
        obj.insert("inputPath".into(), json!(run.input_path));
        obj.insert("configPath".into(), json!(run.config_path));
        obj.insert("buildCommand".into(), json!(cfg.build));
        obj.insert("testCommand".into(), json!(cfg.test));
        obj.insert("benchmarkCommand".into(), json!(cfg.benchmark));
        obj.insert("profileCommand".into(), json!(cfg.profile));
        obj.insert(
            "minImprovementPercent".into(),
            json!(cfg.min_improvement_percent),
        );
        obj.insert(
            "maxRegressionPercent".into(),
            json!(cfg.max_regression_percent),
        );
        obj.insert("logs".into(), json!(run.logs));
        obj.insert("commands".into(), json!(run.commands));
        obj.insert("transformsApplied".into(), json!(run.transforms_applied));
        obj.insert("transformsFailed".into(), json!(run.transforms_failed));
        obj.insert(
            "reproducibleCommand".into(),
            json!(format!(
                "quench-agent optimize --config {}",
                run.config_path
            )),
        );
        obj.insert("status".into(), json!(run.status));
        obj.entry("keptCandidate").or_insert(json!(false));
        obj.insert(
            "disclaimer".into(),
            json!("Local optimization report. Not an ISO certificate, third-party certificate, or certified result."),
        );
    }
    run.report = report;
}

pub fn optimize(cfg: &QuenchConfig) -> Result<PipelineRun, String> {
    validate_for_optimize(cfg)?;
    match cfg.kind {
        ProjectKind::Elf => optimize_elf(cfg),
        ProjectKind::Docker | ProjectKind::Oci => optimize_docker(cfg),
    }
}

fn optimize_docker(cfg: &QuenchConfig) -> Result<PipelineRun, String> {
    let doctor = run_doctor();
    let run_id = new_run_id();
    let run_dir = crate::doctor::work_dir().join("runs").join(&run_id);
    std::fs::create_dir_all(&run_dir).map_err(|e| e.to_string())?;
    let mut run = PipelineRun {
        run_id: run_id.clone(),
        status: RunStatus::Complete,
        created_at: now_ms(),
        project: cfg.project.clone(),
        kind: cfg.kind.as_str().into(),
        input_path: cfg.dockerfile.clone().unwrap_or_default(),
        config_path: cfg.source_path.display().to_string(),
        logs: vec![],
        commands: vec![],
        transforms_applied: vec![],
        transforms_failed: vec![],
        report: json!({}),
    };
    log_run(
        &run_dir,
        &mut run,
        "analyze",
        "Docker/OCI optimize is inspect and recommendation only in this MVP",
    );
    log_run(
        &run_dir,
        &mut run,
        "optimize",
        "Image layers were not rewritten. No container was started.",
    );
    run.transforms_failed.push(Transform {
        tool: "docker rewrite".into(),
        status: "Unavailable".into(),
        detail: "Docker/OCI rewrite is not implemented in this MVP. Inspect and recommendations only. docker_security_args is not applied because no container is executed.".into(),
    });
    if !can_use(&doctor, "docker") {
        run.transforms_failed.push(Transform {
            tool: "docker".into(),
            status: "Unavailable".into(),
            detail: "Docker/containerd is not installed.".into(),
        });
        log_run(&run_dir, &mut run, "rebuild", "docker: Unavailable");
    }
    let recs = vec![
        "Provide a Dockerfile plus build, test, and benchmark commands before any future rewrite path.",
        "Do not treat this report as a rewritten image or as Docker hardening.",
        "Connect a tested container execution path before rewrite is enabled.",
    ];
    finalize_report(
        &mut run,
        cfg,
        json!({
            "ok": true,
            "keptCandidate": false,
            "reason": "No candidate was produced. Docker/OCI is inspect and recommendation only in this MVP; image layers were not rewritten.",
            "testResult": null,
            "toolVersions": tool_versions(&doctor),
            "recommendations": recs,
            "minImprovementPercent": cfg.min_improvement_percent,
            "maxRegressionPercent": cfg.max_regression_percent,
        }),
    );
    write_run(&run_dir, &run)?;
    Ok(run)
}

fn optimize_elf(cfg: &QuenchConfig) -> Result<PipelineRun, String> {
    let doctor = run_doctor();
    if !doctor.supported_platform {
        return Err("Native optimize only supports Linux x86_64".into());
    }
    let run_id = new_run_id();
    let run_dir = crate::doctor::work_dir().join("runs").join(&run_id);
    std::fs::create_dir_all(run_dir.join("artifacts")).map_err(|e| e.to_string())?;
    let mut run = PipelineRun {
        run_id: run_id.clone(),
        status: RunStatus::Running,
        created_at: now_ms(),
        project: cfg.project.clone(),
        kind: "elf".into(),
        input_path: String::new(),
        config_path: cfg.source_path.display().to_string(),
        logs: vec![],
        commands: vec![],
        transforms_applied: vec![],
        transforms_failed: vec![],
        report: json!({}),
    };

    log_run(
        &run_dir,
        &mut run,
        "analyze",
        &format!("config {}", cfg.source_path.display()),
    );
    log_run(&run_dir, &mut run, "analyze", "validate config and input");

    let build_cmd = cfg.build.clone().unwrap();
    log_run(
        &run_dir,
        &mut run,
        "rebuild",
        &format!("build: {build_cmd}"),
    );
    let rec = run_command(
        &build_cmd,
        &cfg.project_root,
        timeout(),
        &[("QUENCH_STAGE", "baseline")],
        None,
    );
    let build_ok = rec.exit_code == 0;
    record(&mut run, rec);
    if !build_ok {
        run.status = RunStatus::Failed;
        finalize_report(
            &mut run,
            cfg,
            json!({
                "ok": false,
                "error": "baseline build failed",
                "toolVersions": tool_versions(&doctor),
                "keptCandidate": false,
            }),
        );
        write_run(&run_dir, &run)?;
        return Ok(run);
    }

    let binary = resolve_in_root(&cfg.project_root, cfg.binary.as_deref().unwrap());
    let binary = std::fs::canonicalize(&binary).unwrap_or(binary);
    if !binary.is_file() {
        run.status = RunStatus::Failed;
        finalize_report(
            &mut run,
            cfg,
            json!({
                "ok": false,
                "error": format!("binary not found after build: {}", binary.display()),
                "keptCandidate": false,
            }),
        );
        write_run(&run_dir, &run)?;
        return Ok(run);
    }
    run.input_path = binary.display().to_string();
    match inspect_path(&binary) {
        InspectOutcome::Ok(ok) => {
            if ok.kind != "elf" {
                return Err("binary is not an ELF".into());
            }
            log_run(
                &run_dir,
                &mut run,
                "analyze",
                &format!("identified ELF sha256={} size={}", ok.sha256, ok.size_bytes),
            );
        }
        InspectOutcome::Err(e) => {
            run.status = RunStatus::Failed;
            finalize_report(
                &mut run,
                cfg,
                json!({"ok": false, "error": e, "keptCandidate": false}),
            );
            write_run(&run_dir, &run)?;
            return Ok(run);
        }
    }

    let baseline_copy = run_dir.join("artifacts/baseline");
    copy_file(&binary, &baseline_copy)?;
    let baseline_sha = sha256_file(&baseline_copy)?;
    let baseline_size = file_size(&baseline_copy)?;

    let test_cmd = cfg.test.clone().unwrap();
    log_run(
        &run_dir,
        &mut run,
        "verify",
        &format!("baseline test: {test_cmd}"),
    );
    let rec = run_command(
        &test_cmd,
        &cfg.project_root,
        timeout(),
        &[
            ("QUENCH_STAGE", "baseline"),
            ("QUENCH_BINARY", &baseline_copy.to_string_lossy()),
        ],
        Some(&baseline_copy),
    );
    let baseline_test_ok = rec.exit_code == 0;
    record(&mut run, rec);
    if !baseline_test_ok {
        run.status = RunStatus::Failed;
        finalize_report(
            &mut run,
            cfg,
            json!({
                "ok": false,
                "error": "baseline tests failed; refusing to optimize",
                "keptCandidate": false,
                "toolVersions": tool_versions(&doctor),
            }),
        );
        write_run(&run_dir, &run)?;
        return Ok(run);
    }
    log_run(
        &run_dir,
        &mut run,
        "verify",
        "Passed the supplied test suite (baseline)",
    );

    let baseline_bench = match run_bench(cfg, &mut run, &run_dir, &baseline_copy, 3, "baseline") {
        Ok(s) => s,
        Err(e) => {
            run.status = RunStatus::Failed;
            finalize_report(
                &mut run,
                cfg,
                json!({"ok": false, "error": e, "keptCandidate": false}),
            );
            write_run(&run_dir, &run)?;
            return Ok(run);
        }
    };

    let mut profile_path: Option<PathBuf> = None;
    if let Some(profile_cmd) = &cfg.profile {
        log_run(&run_dir, &mut run, "profile", profile_cmd);
        if can_use(&doctor, "perf") {
            let perf_data = run_dir.join("artifacts/perf.data");
            let wrapped = format!(
                "perf record --no-buildid --no-buildid-cache -o {} -- {}",
                perf_data.display(),
                profile_cmd
            );
            let rec = run_command(
                &wrapped,
                &cfg.project_root,
                timeout(),
                &[
                    ("QUENCH_BINARY", &baseline_copy.to_string_lossy()),
                    ("QUENCH_STAGE", "profile"),
                ],
                None,
            );
            let ok = rec.exit_code == 0 && perf_data.is_file();
            record(&mut run, rec);
            if ok {
                profile_path = Some(perf_data);
                log_run(&run_dir, &mut run, "profile", "perf profile captured");
            } else {
                log_run(
                    &run_dir,
                    &mut run,
                    "profile",
                    "perf did not produce a usable profile",
                );
            }
        } else {
            let rec = run_command(
                profile_cmd,
                &cfg.project_root,
                timeout(),
                &[
                    ("QUENCH_BINARY", &baseline_copy.to_string_lossy()),
                    ("QUENCH_STAGE", "profile"),
                ],
                Some(&baseline_copy),
            );
            record(&mut run, rec);
            run.transforms_failed.push(Transform {
                tool: "perf".into(),
                status: "Unavailable".into(),
                detail: "perf is not installed; no LBR profile was collected".into(),
            });
            log_run(&run_dir, &mut run, "profile", "perf: Unavailable");
        }
    }

    let mut current = baseline_copy.clone();
    if can_use(&doctor, "llvm-bolt") {
        if let Some(profile) = &profile_path {
            let bolted = run_dir.join("artifacts/candidate.bolt");
            let cmd = format!(
                "llvm-bolt {} -o {} -data={} -reorder-blocks=ext-tsp -reorder-functions=hfsort -split-functions -split-all-cold -dyno-stats",
                current.display(),
                bolted.display(),
                profile.display()
            );
            log_run(&run_dir, &mut run, "optimize", &cmd);
            let rec = run_command(&cmd, &cfg.project_root, timeout(), &[], Some(&bolted));
            let ok = rec.exit_code == 0 && bolted.is_file();
            record(&mut run, rec);
            if ok {
                run.transforms_applied.push(Transform {
                    tool: "llvm-bolt".into(),
                    status: "applied".into(),
                    detail: "created BOLT candidate from profile".into(),
                });
                current = bolted;
            } else {
                run.transforms_failed.push(Transform {
                    tool: "llvm-bolt".into(),
                    status: "failed".into(),
                    detail: "llvm-bolt ran but did not produce a candidate".into(),
                });
            }
        } else {
            run.transforms_failed.push(Transform {
                tool: "llvm-bolt".into(),
                status: "skipped".into(),
                detail: "no valid profile; llvm-bolt was not applied".into(),
            });
            log_run(
                &run_dir,
                &mut run,
                "optimize",
                "llvm-bolt skipped: no valid profile",
            );
        }
    } else {
        run.transforms_failed.push(Transform {
            tool: "llvm-bolt".into(),
            status: "Unavailable".into(),
            detail: "llvm-bolt not found on PATH; no layout rewrite was performed".into(),
        });
        log_run(&run_dir, &mut run, "optimize", "llvm-bolt: Unavailable");
    }

    if can_use(&doctor, "strip") {
        let stripped = run_dir.join("artifacts/candidate.stripped");
        copy_file(&current, &stripped)?;
        let cmd = format!("strip --strip-unneeded {}", stripped.display());
        log_run(&run_dir, &mut run, "optimize", &cmd);
        let rec = run_command(&cmd, &run_dir, timeout(), &[], Some(&stripped));
        let ok = rec.exit_code == 0 && stripped.is_file();
        record(&mut run, rec);
        if ok {
            run.transforms_applied.push(Transform {
                tool: "strip".into(),
                status: "applied".into(),
                detail: "strip --strip-unneeded on a copy".into(),
            });
            current = stripped;
        } else {
            run.transforms_failed.push(Transform {
                tool: "strip".into(),
                status: "failed".into(),
                detail: "strip failed; copy discarded".into(),
            });
        }
    } else {
        run.transforms_failed.push(Transform {
            tool: "strip".into(),
            status: "Unavailable".into(),
            detail: "strip not found on PATH".into(),
        });
    }

    if current == baseline_copy {
        run.status = RunStatus::Complete;
        log_run(
            &run_dir,
            &mut run,
            "prove",
            "no candidate produced; baseline retained",
        );
        finalize_report(
            &mut run,
            cfg,
            json!({
                "ok": true,
                "keptCandidate": false,
                "reason": "No candidate was produced. Missing tools are listed as Unavailable; success was not invented.",
                "inputPath": binary.display().to_string(),
                "buildCommand": cfg.build,
                "testCommand": cfg.test,
                "benchmarkCommand": cfg.benchmark,
                "toolVersions": tool_versions(&doctor),
                "baselineSha256": baseline_sha,
                "candidateSha256": Value::Null,
                "artifactSize": { "baselineBytes": baseline_size, "candidateBytes": Value::Null },
                "testResult": "Passed the supplied test suite",
                "benchmarkRepetitions": baseline_bench.repetitions,
                "medianMs": { "baseline": baseline_bench.median_ms, "candidate": Value::Null },
                "p95Ms": { "baseline": baseline_bench.p95_ms, "candidate": Value::Null },
                "baseline": artifact_meta(&baseline_copy),
                "runtime": { "baseline": baseline_bench, "candidate": Value::Null },
                "minImprovementPercent": cfg.min_improvement_percent,
                "maxRegressionPercent": cfg.max_regression_percent,
            }),
        );
        write_run(&run_dir, &run)?;
        return Ok(run);
    }

    let candidate_path = run_dir.join("artifacts/candidate");
    copy_file(&current, &candidate_path)?;
    let candidate_sha = sha256_file(&candidate_path)?;
    let candidate_size = file_size(&candidate_path)?;

    log_run(&run_dir, &mut run, "verify", "candidate test");
    let rec = run_command(
        &test_cmd,
        &cfg.project_root,
        timeout(),
        &[
            ("QUENCH_STAGE", "candidate"),
            ("QUENCH_BINARY", &candidate_path.to_string_lossy()),
        ],
        Some(&candidate_path),
    );
    let cand_test_ok = rec.exit_code == 0;
    record(&mut run, rec);
    if !cand_test_ok {
        run.status = RunStatus::VerificationFailed;
        log_run(
            &run_dir,
            &mut run,
            "verify",
            "candidate discarded: supplied test suite failed",
        );
        let _ = std::fs::remove_file(&candidate_path);
        finalize_report(
            &mut run,
            cfg,
            json!({
                "ok": false,
                "keptCandidate": false,
                "reason": "Candidate discarded because the supplied test suite failed",
                "testResult": "Candidate discarded — supplied test suite failed",
                "inputPath": binary.display().to_string(),
                "buildCommand": cfg.build,
                "testCommand": cfg.test,
                "benchmarkCommand": cfg.benchmark,
                "toolVersions": tool_versions(&doctor),
                "baselineSha256": baseline_sha,
                "candidateSha256": candidate_sha,
                "artifactSize": { "baselineBytes": baseline_size, "candidateBytes": candidate_size },
                "runtime": { "baseline": baseline_bench, "candidate": Value::Null },
            }),
        );
        write_run(&run_dir, &run)?;
        return Ok(run);
    }
    log_run(
        &run_dir,
        &mut run,
        "verify",
        "Passed the supplied test suite",
    );

    let candidate_bench = match run_bench(cfg, &mut run, &run_dir, &candidate_path, 3, "candidate")
    {
        Ok(s) => s,
        Err(e) => {
            run.status = RunStatus::Failed;
            finalize_report(
                &mut run,
                cfg,
                json!({"ok": false, "error": e, "keptCandidate": false}),
            );
            write_run(&run_dir, &run)?;
            return Ok(run);
        }
    };

    if let Some(reason) = should_discard_candidate(
        &baseline_bench,
        &candidate_bench,
        cfg.max_regression_percent,
        cfg.min_improvement_percent,
    ) {
        run.status = RunStatus::Failed;
        log_run(
            &run_dir,
            &mut run,
            "benchmark",
            &format!("candidate discarded: {reason}"),
        );
        let _ = std::fs::remove_file(&candidate_path);
        finalize_report(
            &mut run,
            cfg,
            json!({
                "ok": false,
                "keptCandidate": false,
                "reason": reason,
                "testResult": "Passed the supplied test suite",
                "inputPath": binary.display().to_string(),
                "buildCommand": cfg.build,
                "testCommand": cfg.test,
                "benchmarkCommand": cfg.benchmark,
                "toolVersions": tool_versions(&doctor),
                "baselineSha256": baseline_sha,
                "candidateSha256": candidate_sha,
                "artifactSize": { "baselineBytes": baseline_size, "candidateBytes": candidate_size },
                "benchmarkRepetitions": candidate_bench.repetitions,
                "medianMs": { "baseline": baseline_bench.median_ms, "candidate": candidate_bench.median_ms },
                "p95Ms": { "baseline": baseline_bench.p95_ms, "candidate": candidate_bench.p95_ms },
                "runtime": { "baseline": baseline_bench, "candidate": candidate_bench },
                "minImprovementPercent": cfg.min_improvement_percent,
                "maxRegressionPercent": cfg.max_regression_percent,
                "medianImprovementPercent": median_improvement_pct(baseline_bench.median_ms, candidate_bench.median_ms),
            }),
        );
        write_run(&run_dir, &run)?;
        return Ok(run);
    }

    run.status = RunStatus::Complete;
    let med_impr = median_improvement_pct(baseline_bench.median_ms, candidate_bench.median_ms);
    log_run(
        &run_dir,
        &mut run,
        "prove",
        &format!(
            "kept candidate size {} -> {}  median_ms {:.4} -> {:.4} ({med_impr:.3}%)",
            baseline_size, candidate_size, baseline_bench.median_ms, candidate_bench.median_ms
        ),
    );
    finalize_report(
        &mut run,
        cfg,
        json!({
            "ok": true,
            "keptCandidate": true,
            "reason": "Candidate kept: supplied test suite passed, runtime did not exceed max_regression_percent, and median improvement met min_improvement_percent",
            "testResult": "Passed the supplied test suite",
            "inputPath": binary.display().to_string(),
            "buildCommand": cfg.build,
            "testCommand": cfg.test,
            "benchmarkCommand": cfg.benchmark,
            "toolVersions": tool_versions(&doctor),
            "baselineSha256": baseline_sha,
            "candidateSha256": candidate_sha,
            "artifactSize": { "baselineBytes": baseline_size, "candidateBytes": candidate_size },
            "benchmarkRepetitions": candidate_bench.repetitions,
            "medianMs": { "baseline": baseline_bench.median_ms, "candidate": candidate_bench.median_ms },
            "p95Ms": { "baseline": baseline_bench.p95_ms, "candidate": candidate_bench.p95_ms },
            "runtime": { "baseline": baseline_bench, "candidate": candidate_bench },
            "baseline": artifact_meta(&baseline_copy),
            "candidate": artifact_meta(&candidate_path),
            "minImprovementPercent": cfg.min_improvement_percent,
            "maxRegressionPercent": cfg.max_regression_percent,
            "medianImprovementPercent": med_impr,
        }),
    );
    write_run(&run_dir, &run)?;
    Ok(run)
}

pub fn write_run(run_dir: &Path, run: &PipelineRun) -> Result<(), String> {
    write_json(
        &run_dir.join("run.json"),
        &serde_json::to_value(run).unwrap_or(json!({})),
    )?;
    write_json(&run_dir.join("report.json"), &run.report)?;
    write_json(
        &run_dir.join("provenance.json"),
        &json!({
            "runId": run.run_id,
            "createdAt": run.created_at,
            "commands": run.commands,
            "transformsApplied": run.transforms_applied,
            "transformsFailed": run.transforms_failed,
            "report": run.report,
        }),
    )?;
    Ok(())
}

pub fn load_run(run_id: &str) -> Result<PipelineRun, String> {
    let path = crate::doctor::work_dir()
        .join("runs")
        .join(run_id)
        .join("run.json");
    let text = std::fs::read_to_string(&path).map_err(|_| format!("run not found: {run_id}"))?;
    serde_json::from_str(&text).map_err(|e| format!("invalid run json: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::load_config;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn setup_fixture(name: &str) -> (PathBuf, PathBuf, std::sync::MutexGuard<'static, ()>) {
        let guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = std::env::temp_dir().join(format!(
            "quench-fix-{}-{}-{}",
            name,
            std::process::id(),
            now_ms()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("testdata")
            .join(name);
        for ent in std::fs::read_dir(&src).unwrap() {
            let ent = ent.unwrap();
            let dest = tmp.join(ent.file_name());
            std::fs::copy(ent.path(), &dest).unwrap();
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for script in ["test.sh", "bench.sh"] {
                let p = tmp.join(script);
                if p.exists() {
                    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
                }
            }
        }
        let home = tmp.join(".quench-home");
        std::fs::create_dir_all(&home).unwrap();
        std::env::set_var("QUENCH_HOME", &home);
        std::env::set_var("QUENCH_CMD_TIMEOUT_SECS", "60");
        std::env::remove_var("QUENCH_FORCE_UNAVAILABLE");
        (tmp, home, guard)
    }

    #[test]
    fn discards_on_median_regression() {
        let b = stats_from_samples(vec![100.0, 100.0, 100.0]);
        let c = stats_from_samples(vec![110.0, 111.0, 109.0]);
        let reason = should_discard_regression(&b, &c, 2.0).expect("should discard");
        assert!(reason.contains("median"));
    }

    #[test]
    fn keeps_within_threshold() {
        let b = stats_from_samples(vec![100.0, 100.0, 100.0]);
        let c = stats_from_samples(vec![101.0, 100.5, 101.2]);
        assert!(should_discard_regression(&b, &c, 2.0).is_none());
    }

    #[test]
    fn discards_on_p95_regression() {
        let b = stats_from_samples(vec![100.0, 100.0, 100.0, 100.0, 100.0]);
        let c = stats_from_samples(vec![100.0, 100.0, 100.0, 100.0, 130.0]);
        let reason = should_discard_regression(&b, &c, 2.0).expect("p95");
        assert!(reason.contains("p95"));
    }

    #[test]
    fn min_improvement_percent_discards_small_gains() {
        let b = stats_from_samples(vec![100.0, 100.0, 100.0]);
        let c = stats_from_samples(vec![99.5, 99.6, 99.4]);
        let reason = should_discard_candidate(&b, &c, 2.0, 1.0).expect("below min improvement");
        assert!(reason.contains("min_improvement_percent"));
        assert!(should_discard_candidate(&b, &c, 2.0, 0.0).is_none());
    }

    #[test]
    fn candidate_test_failure_discards() {
        let (root, _home, _guard) = setup_fixture("fail-test");
        let cfg = load_config(&root.join("quench.yaml")).unwrap();
        let run = optimize(&cfg).expect("optimize should return a report");
        assert_core_native_report(&run);
        assert_eq!(run.status, RunStatus::VerificationFailed);
        assert_eq!(run.report["keptCandidate"], false);
        let test_result = run.report["testResult"].as_str().unwrap_or("");
        assert!(
            test_result.to_lowercase().contains("fail"),
            "testResult={test_result}"
        );
        assert!(
            run.transforms_failed
                .iter()
                .any(|t| t.tool == "llvm-bolt" && t.status == "Unavailable")
                || run.transforms_applied.iter().any(|t| t.tool == "strip")
        );
    }

    #[test]
    fn benchmark_regression_discards() {
        let (root, _home, _guard) = setup_fixture("regression");
        let cfg = load_config(&root.join("quench.yaml")).unwrap();
        let run = optimize(&cfg).expect("optimize should return a report");
        assert_core_native_report(&run);
        assert_eq!(run.status, RunStatus::Failed);
        assert_eq!(run.report["keptCandidate"], false);
        let reason = run.report["reason"].as_str().unwrap_or("");
        assert!(
            reason.contains("regression") || reason.contains("min_improvement"),
            "reason={reason}"
        );
        assert_eq!(
            run.report["testResult"].as_str().unwrap(),
            "Passed the supplied test suite"
        );
    }

    #[test]
    fn missing_llvm_bolt_is_unavailable() {
        let (root, _home, _guard) = setup_fixture("regression");
        std::env::set_var("QUENCH_FORCE_UNAVAILABLE", "llvm-bolt,perf");
        let cfg = load_config(&root.join("quench.yaml")).unwrap();
        let run = optimize(&cfg).expect("optimize should return a report");
        assert_core_native_report(&run);
        let bolt = run
            .transforms_failed
            .iter()
            .find(|t| t.tool == "llvm-bolt")
            .expect("llvm-bolt must be recorded");
        assert_eq!(bolt.status, "Unavailable");
        assert!(
            bolt.detail.to_lowercase().contains("not found")
                || bolt.detail.contains("Unavailable")
                || bolt.detail.to_lowercase().contains("not")
        );
    }

    #[test]
    fn no_candidate_when_tools_unavailable() {
        let (root, _home, _guard) = setup_fixture("regression");
        std::env::set_var("QUENCH_FORCE_UNAVAILABLE", "llvm-bolt,perf,strip");
        let cfg = load_config(&root.join("quench.yaml")).unwrap();
        let run = optimize(&cfg).expect("optimize should return a report");
        assert_core_native_report(&run);
        assert_eq!(run.status, RunStatus::Complete);
        assert_eq!(run.report["keptCandidate"], false);
        let reason = run.report["reason"].as_str().unwrap_or("");
        assert!(
            reason.to_lowercase().contains("no candidate"),
            "reason={reason}"
        );
        assert!(run.transforms_applied.is_empty());
        assert!(run
            .transforms_failed
            .iter()
            .any(|t| t.tool == "strip" && t.status == "Unavailable"));
        assert!(run
            .transforms_failed
            .iter()
            .any(|t| t.tool == "llvm-bolt" && t.status == "Unavailable"));
        assert_eq!(
            run.report["testResult"].as_str().unwrap(),
            "Passed the supplied test suite"
        );
    }

    fn assert_core_native_report(run: &PipelineRun) {
        let r = &run.report;
        assert!(
            r.get("buildCommand").and_then(|v| v.as_str()).is_some(),
            "buildCommand"
        );
        assert!(
            r.get("testCommand").and_then(|v| v.as_str()).is_some(),
            "testCommand"
        );
        assert!(
            r.get("benchmarkCommand").and_then(|v| v.as_str()).is_some(),
            "benchmarkCommand"
        );
        assert!(r.get("profileCommand").is_some(), "profileCommand");
        assert!(r
            .get("minImprovementPercent")
            .and_then(|v| v.as_f64())
            .is_some());
        assert!(r
            .get("maxRegressionPercent")
            .and_then(|v| v.as_f64())
            .is_some());
        assert!(r.get("keptCandidate").and_then(|v| v.as_bool()).is_some());
        assert!(r.get("toolVersions").and_then(|v| v.as_object()).is_some());
        assert!(r.get("project").and_then(|v| v.as_str()).is_some());
        assert!(r
            .get("reproducibleCommand")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .contains("optimize --config"));
    }

    #[test]
    fn keeps_candidate_when_median_improvement_meets_threshold() {
        let (root, _home, _guard) = setup_fixture("success");
        let cfg = load_config(&root.join("quench.yaml")).unwrap();
        let run = optimize(&cfg).expect("optimize should return a report");
        assert_core_native_report(&run);
        assert_eq!(run.status, RunStatus::Complete);
        assert_eq!(run.report["ok"], true);
        assert_eq!(run.report["keptCandidate"], true);
        assert!(run.report["baselineSha256"].as_str().unwrap_or("").len() >= 16);
        assert!(run.report["candidateSha256"].as_str().unwrap_or("").len() >= 16);
        let impr = run.report["medianImprovementPercent"]
            .as_f64()
            .expect("medianImprovementPercent");
        assert!(impr >= 1.0, "impr={impr}");
        assert_eq!(run.report["minImprovementPercent"], 1.0);
        assert_eq!(run.report["maxRegressionPercent"], 2.0);
        assert_eq!(
            run.report["testResult"].as_str().unwrap(),
            "Passed the supplied test suite"
        );
        assert!(run
            .transforms_applied
            .iter()
            .any(|t| t.tool == "strip" && t.status == "applied"));
    }
}
