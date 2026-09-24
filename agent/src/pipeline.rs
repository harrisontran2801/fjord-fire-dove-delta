use crate::config::{validate_for_optimize, ProjectKind, QuenchConfig};
use crate::doctor::{run_doctor, tool_available, DoctorReport};
use crate::exec::{resolve_in_root, run_command, tool_version, write_log_line, CommandRecord};
use crate::inspect::{inspect_path, InspectOutcome};
use crate::profile::{
    bolt_optimize_command, elf_has_text_relocs, find_bolt_rt_instr, forced_profile_mode,
    instrument_command, lbr_record_command, nl_record_command, probe_lbr,
    rewrite_profile_command_for_instrumented, select_profile_mode, ProfileInfo, ProfileInputs,
    ProfileMode,
};
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
    #[serde(default)]
    pub min_ms: f64,
    #[serde(default)]
    pub max_ms: f64,
    #[serde(default)]
    pub spread_ms: f64,
    #[serde(default)]
    pub stddev_ms: f64,
    #[serde(default)]
    pub warmup: usize,
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
    #[serde(default)]
    pub profile: ProfileInfo,
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
    let min_ms = samples.first().copied().unwrap_or(0.0);
    let max_ms = samples.last().copied().unwrap_or(0.0);
    let mean = if repetitions == 0 {
        0.0
    } else {
        samples.iter().sum::<f64>() / repetitions as f64
    };
    let stddev_ms = if repetitions < 2 {
        0.0
    } else {
        let var =
            samples.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / (repetitions as f64 - 1.0);
        var.sqrt()
    };
    BenchStats {
        median_ms: median(&samples),
        p95_ms: percentile(&samples, 95.0),
        repetitions,
        samples_ms: samples,
        min_ms,
        max_ms,
        spread_ms: max_ms - min_ms,
        stddev_ms,
        warmup: 0,
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

fn env_bounded(key: &str, fallback: u32, min: u32, max: u32) -> Result<usize, String> {
    let n = match std::env::var(key) {
        Ok(s) if !s.trim().is_empty() => s
            .trim()
            .parse::<u32>()
            .map_err(|_| format!("invalid {key}: {s}"))?,
        _ => fallback,
    };
    if n < min || n > max {
        return Err(format!("{key}={n} is outside {min}..={max}"));
    }
    Ok(n as usize)
}

fn bench_plan(cfg: &QuenchConfig) -> Result<(usize, usize), String> {
    let repetitions = env_bounded("QUENCH_BENCH_REPETITIONS", cfg.benchmark_repetitions, 1, 30)?;
    let warmup = env_bounded("QUENCH_BENCH_WARMUP", cfg.benchmark_warmup, 0, 10)?;
    Ok((repetitions, warmup))
}

fn stability_warning(baseline: &BenchStats, candidate: Option<&BenchStats>) -> Option<String> {
    let mut notes = Vec::new();
    if baseline.repetitions < 10 {
        notes.push(format!(
            "only {} measured repetitions; p95 is less stable below 10",
            baseline.repetitions
        ));
    }
    let mut sides = vec![("baseline", baseline)];
    if let Some(c) = candidate {
        sides.push(("candidate", c));
    }
    for (name, stats) in sides {
        if stats.median_ms > 0.0 {
            let pct = (stats.spread_ms / stats.median_ms) * 100.0;
            if pct > 5.0 {
                notes.push(format!("{name} spread is {pct:.1}% of median"));
            }
        }
    }
    if notes.is_empty() {
        None
    } else {
        Some(format!(
            "{}. This warning does not change the 1% / 2% gates.",
            notes.join("; ")
        ))
    }
}

fn run_one_bench(
    cfg: &QuenchConfig,
    run: &mut PipelineRun,
    binary: &Path,
    cmd: &str,
    stage: &str,
) -> Result<f64, String> {
    let rec = run_command(
        cmd,
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
    let elapsed = parse_elapsed_ms(&rec.stdout, rec.duration_ms);
    record(run, rec);
    Ok(elapsed)
}

fn run_bench(
    cfg: &QuenchConfig,
    run: &mut PipelineRun,
    run_dir: &Path,
    binary: &Path,
    stage: &str,
) -> Result<BenchStats, String> {
    let cmd = cfg.benchmark.clone().unwrap();
    let (times, warmup) = bench_plan(cfg)?;
    for i in 1..=warmup {
        log_run(
            run_dir,
            run,
            "benchmark",
            &format!("{stage} warmup {i}/{warmup} (excluded)"),
        );
        let ms = run_one_bench(cfg, run, binary, &cmd, stage)?;
        log_run(
            run_dir,
            run,
            "benchmark",
            &format!("{stage} warmup[{i}] elapsed_ms={ms:.4} excluded"),
        );
    }
    let mut samples = Vec::new();
    for i in 1..=times {
        log_run(
            run_dir,
            run,
            "benchmark",
            &format!("{stage} repetition {i}/{times}"),
        );
        let ms = run_one_bench(cfg, run, binary, &cmd, stage)?;
        samples.push(ms);
        log_run(
            run_dir,
            run,
            "benchmark",
            &format!("{stage}[{i}] elapsed_ms={ms:.4}"),
        );
    }
    let mut stats = stats_from_samples(samples);
    stats.warmup = warmup;
    Ok(stats)
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
        let p = run.profile.to_json();
        if let Some(pmap) = p.as_object() {
            for (k, v) in pmap {
                obj.entry(k.clone()).or_insert(v.clone());
            }
        }
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
        profile: ProfileInfo::default(),
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
        profile: ProfileInfo::default(),
    };

    log_run(
        &run_dir,
        &mut run,
        "analyze",
        &format!("config {}", cfg.source_path.display()),
    );
    log_run(&run_dir, &mut run, "analyze", "validate config and input");

    let pf = crate::preflight::evaluate(cfg, &doctor);
    if !pf.ok {
        run.status = RunStatus::Failed;
        let error = pf.blocking_summary();
        log_run(&run_dir, &mut run, "analyze", &error);
        finalize_report(
            &mut run,
            cfg,
            json!({
                "ok": false,
                "error": error,
                "keptCandidate": false,
                "toolVersions": tool_versions(&doctor),
            }),
        );
        write_run(&run_dir, &run)?;
        return Ok(run);
    }

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

    let baseline_bench = match run_bench(cfg, &mut run, &run_dir, &baseline_copy, "baseline") {
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
    collect_profile(
        cfg,
        &mut run,
        &run_dir,
        &doctor,
        &baseline_copy,
        &mut profile_path,
    );
    run.profile.benchmarked_original = true;
    run.profile.benchmarked_instrumented = false;

    let mut current = baseline_copy.clone();
    apply_bolt_if_profiled(
        &mut run,
        &run_dir,
        &doctor,
        &baseline_copy,
        &mut current,
        profile_path.as_deref(),
    );

    let bolt_applied = run
        .transforms_applied
        .iter()
        .any(|t| t.tool == "llvm-bolt" && t.status == "applied");
    if can_use(&doctor, "strip") {
        if bolt_applied {
            run.transforms_failed.push(Transform {
                tool: "strip".into(),
                status: "skipped".into(),
                detail: "GNU strip is not applied after llvm-bolt; it can break BOLT section layout. The BOLT candidate is tested unstripped.".into(),
            });
            log_run(
                &run_dir,
                &mut run,
                "optimize",
                "strip skipped after llvm-bolt (section layout)",
            );
        } else {
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
                "benchmarkWarmup": baseline_bench.warmup,
                "medianMs": { "baseline": baseline_bench.median_ms, "candidate": Value::Null },
                "p95Ms": { "baseline": baseline_bench.p95_ms, "candidate": Value::Null },
                "minMs": { "baseline": baseline_bench.min_ms, "candidate": Value::Null },
                "maxMs": { "baseline": baseline_bench.max_ms, "candidate": Value::Null },
                "spreadMs": { "baseline": baseline_bench.spread_ms, "candidate": Value::Null },
                "stdDevMs": { "baseline": baseline_bench.stddev_ms, "candidate": Value::Null },
                "stabilityWarning": stability_warning(&baseline_bench, None),
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

    let candidate_bench = match run_bench(cfg, &mut run, &run_dir, &candidate_path, "candidate") {
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
    run.profile.benchmarked_candidate = true;
    run.profile.benchmarked_instrumented = false;

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
                "benchmarkWarmup": candidate_bench.warmup,
                "medianMs": { "baseline": baseline_bench.median_ms, "candidate": candidate_bench.median_ms },
                "p95Ms": { "baseline": baseline_bench.p95_ms, "candidate": candidate_bench.p95_ms },
                "minMs": { "baseline": baseline_bench.min_ms, "candidate": candidate_bench.min_ms },
                "maxMs": { "baseline": baseline_bench.max_ms, "candidate": candidate_bench.max_ms },
                "spreadMs": { "baseline": baseline_bench.spread_ms, "candidate": candidate_bench.spread_ms },
                "stdDevMs": { "baseline": baseline_bench.stddev_ms, "candidate": candidate_bench.stddev_ms },
                "stabilityWarning": stability_warning(&baseline_bench, Some(&candidate_bench)),
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
            "benchmarkWarmup": candidate_bench.warmup,
            "medianMs": { "baseline": baseline_bench.median_ms, "candidate": candidate_bench.median_ms },
            "p95Ms": { "baseline": baseline_bench.p95_ms, "candidate": candidate_bench.p95_ms },
            "minMs": { "baseline": baseline_bench.min_ms, "candidate": candidate_bench.min_ms },
            "maxMs": { "baseline": baseline_bench.max_ms, "candidate": candidate_bench.max_ms },
            "spreadMs": { "baseline": baseline_bench.spread_ms, "candidate": candidate_bench.spread_ms },
            "stdDevMs": { "baseline": baseline_bench.stddev_ms, "candidate": candidate_bench.stddev_ms },
            "stabilityWarning": stability_warning(&baseline_bench, Some(&candidate_bench)),
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

fn collect_profile(
    cfg: &QuenchConfig,
    run: &mut PipelineRun,
    run_dir: &Path,
    doctor: &DoctorReport,
    baseline: &Path,
    profile_path: &mut Option<PathBuf>,
) {
    let has_relocs = elf_has_text_relocs(baseline).unwrap_or(false);
    let bolt_rt = find_bolt_rt_instr();
    let probe = probe_lbr();
    run.profile.lbr_probe_ok = Some(probe.ok);
    run.profile.lbr_probe_command = probe.command.clone();
    run.profile.lbr_probe_detail = probe.detail.clone();
    run.profile.bolt_rt = bolt_rt.as_ref().map(|p| p.display().to_string());
    run.profile.has_text_relocs = has_relocs;
    log_run(
        run_dir,
        run,
        "profile",
        &format!("LBR probe: {} ({})", probe.command, probe.detail),
    );

    let decision = select_profile_mode(ProfileInputs {
        lbr_probe_ok: probe.ok,
        perf_ok: can_use(doctor, "perf"),
        bolt_ok: can_use(doctor, "llvm-bolt"),
        bolt_rt: bolt_rt.clone(),
        has_text_relocs: has_relocs,
        has_profile_cmd: cfg.profile.is_some(),
        force: forced_profile_mode(),
    });
    run.profile.mode = decision.mode;
    run.profile.reason = decision.reason.clone();
    run.profile.warning = decision.warning.clone();
    log_run(
        run_dir,
        run,
        "profile",
        &format!("profileMode={} {}", decision.mode.as_str(), decision.reason),
    );
    if let Some(w) = &decision.warning {
        log_run(run_dir, run, "profile", w);
    }

    if !can_use(doctor, "llvm-bolt") {
        run.transforms_failed.push(Transform {
            tool: "llvm-bolt".into(),
            status: "Unavailable".into(),
            detail: "llvm-bolt not found on PATH; no layout rewrite was performed".into(),
        });
        log_run(run_dir, run, "optimize", "llvm-bolt: Unavailable");
        // LBR collection does not need the bolt binary. Record the real perf
        // command even when a later rewrite is impossible. Every other mode
        // stops here, and a forced instrument mode must not stay "instrument".
        if decision.mode != ProfileMode::Lbr {
            if decision.mode == ProfileMode::Instrument {
                run.profile.mode = ProfileMode::Unavailable;
                run.profile.reason = if bolt_rt.is_none() {
                    "libbolt_rt_instr.a missing".into()
                } else {
                    "llvm-bolt is Unavailable; instrumentation was not run".into()
                };
            }
            return;
        }
    }

    let Some(profile_cmd) = cfg.profile.clone() else {
        run.transforms_failed.push(Transform {
            tool: "llvm-bolt".into(),
            status: "skipped".into(),
            detail: "no profile command; llvm-bolt was not applied".into(),
        });
        return;
    };

    match decision.mode {
        ProfileMode::Lbr => {
            let perf_data = run_dir.join("artifacts/perf.data");
            let wrapped = lbr_record_command(&perf_data, &profile_cmd);
            log_run(run_dir, run, "profile", &wrapped);
            let rec = run_command(
                &wrapped,
                &cfg.project_root,
                timeout(),
                &[
                    ("QUENCH_BINARY", &baseline.to_string_lossy()),
                    ("QUENCH_STAGE", "profile"),
                ],
                None,
            );
            let ok =
                rec.exit_code == 0 && perf_data.is_file() && file_size(&perf_data).unwrap_or(0) > 0;
            record(run, rec);
            if ok {
                *profile_path = Some(perf_data);
                log_run(run_dir, run, "profile", "LBR perf profile captured");
            } else {
                log_run(
                    run_dir,
                    run,
                    "profile",
                    "LBR perf record failed; no profile",
                );
                run.profile.reason = "LBR collection failed; no profile was produced".into();
            }
        }
        ProfileMode::Instrument => {
            if bolt_rt.is_none() {
                run.transforms_failed.push(Transform {
                    tool: "llvm-bolt".into(),
                    status: "Unavailable".into(),
                    detail: "libbolt_rt_instr.a not found; instrumentation fallback skipped".into(),
                });
                run.profile.mode = ProfileMode::Unavailable;
                run.profile.reason = "libbolt_rt_instr.a missing".into();
                return;
            }
            if !has_relocs {
                run.transforms_failed.push(Transform {
                    tool: "llvm-bolt".into(),
                    status: "skipped".into(),
                    detail: "binary has no .rela.text; rebuild with -Wl,--emit-relocs".into(),
                });
                log_run(
                    run_dir,
                    run,
                    "optimize",
                    "llvm-bolt skipped: missing text relocations",
                );
                return;
            }
            let inst = run_dir.join("artifacts/instrumented");
            let fdata = run_dir.join("artifacts/prof.fdata");
            let _ = std::fs::remove_file(&fdata);
            let cmd = instrument_command(baseline, &inst, &fdata);
            log_run(run_dir, run, "profile", &cmd);
            let rec = run_command(&cmd, &cfg.project_root, timeout(), &[], Some(&inst));
            let inst_ok = rec.exit_code == 0 && inst.is_file();
            record(run, rec);
            if !inst_ok {
                run.transforms_failed.push(Transform {
                    tool: "llvm-bolt".into(),
                    status: "failed".into(),
                    detail: "BOLT instrumentation did not produce an instrumented copy".into(),
                });
                let _ = std::fs::remove_file(&inst);
                return;
            }
            run.profile.instrumented_path = Some(inst.display().to_string());
            let project_bin =
                resolve_in_root(&cfg.project_root, cfg.binary.as_deref().unwrap_or(""));
            let workload = rewrite_profile_command_for_instrumented(
                &profile_cmd,
                baseline,
                &project_bin,
                &inst,
                &cfg.project_root,
            );
            log_run(
                run_dir,
                run,
                "profile",
                &format!(
                    "running workload against instrumented copy (not a benchmark): {workload}"
                ),
            );
            let rec = run_command(
                &workload,
                &cfg.project_root,
                timeout(),
                &[
                    ("QUENCH_BINARY", &inst.to_string_lossy()),
                    ("QUENCH_STAGE", "profile"),
                ],
                Some(&inst),
            );
            record(run, rec);
            let fdata_ok = fdata.is_file() && file_size(&fdata).unwrap_or(0) > 0;
            let _ = std::fs::remove_file(&inst);
            run.profile.instrumented_path = None;
            if fdata_ok {
                *profile_path = Some(fdata.clone());
                run.profile.fdata_path = Some(fdata.display().to_string());
                log_run(run_dir, run, "profile", "instrumentation fdata captured");
            } else {
                run.transforms_failed.push(Transform {
                    tool: "llvm-bolt".into(),
                    status: "failed".into(),
                    detail: "instrumented workload did not write a usable .fdata profile".into(),
                });
            }
        }
        ProfileMode::Nl => {
            let perf_data = run_dir.join("artifacts/perf.data");
            let wrapped = nl_record_command(&perf_data, &profile_cmd);
            log_run(run_dir, run, "profile", &wrapped);
            let rec = run_command(
                &wrapped,
                &cfg.project_root,
                timeout(),
                &[
                    ("QUENCH_BINARY", &baseline.to_string_lossy()),
                    ("QUENCH_STAGE", "profile"),
                ],
                None,
            );
            let ok = rec.exit_code == 0 && perf_data.is_file();
            record(run, rec);
            if ok {
                *profile_path = Some(perf_data);
                log_run(run_dir, run, "profile", "no-LBR perf profile captured");
            } else {
                log_run(run_dir, run, "profile", "no-LBR perf record failed");
            }
        }
        ProfileMode::Unavailable => {
            if !can_use(doctor, "perf") {
                run.transforms_failed.push(Transform {
                    tool: "perf".into(),
                    status: "Unavailable".into(),
                    detail: "perf is not installed; no LBR profile was collected".into(),
                });
            }
            run.transforms_failed.push(Transform {
                tool: "llvm-bolt".into(),
                status: "skipped".into(),
                detail: decision.reason.clone(),
            });
        }
    }
}

fn apply_bolt_if_profiled(
    run: &mut PipelineRun,
    run_dir: &Path,
    doctor: &DoctorReport,
    baseline: &Path,
    current: &mut PathBuf,
    profile: Option<&Path>,
) {
    if !can_use(doctor, "llvm-bolt") {
        return;
    }
    let Some(profile) = profile else {
        if !run.transforms_failed.iter().any(|t| t.tool == "llvm-bolt") {
            run.transforms_failed.push(Transform {
                tool: "llvm-bolt".into(),
                status: "skipped".into(),
                detail: "no valid profile; llvm-bolt was not applied".into(),
            });
            log_run(
                run_dir,
                run,
                "optimize",
                "llvm-bolt skipped: no valid profile",
            );
        }
        return;
    };
    if !run.profile.has_text_relocs && run.profile.mode != ProfileMode::Lbr {
        if !run
            .transforms_failed
            .iter()
            .any(|t| t.tool == "llvm-bolt" && t.status == "skipped")
        {
            run.transforms_failed.push(Transform {
                tool: "llvm-bolt".into(),
                status: "skipped".into(),
                detail: "binary has no .rela.text; rebuild with -Wl,--emit-relocs".into(),
            });
        }
        return;
    }
    let bolted = run_dir.join("artifacts/candidate.bolt");
    let cmd = bolt_optimize_command(
        baseline,
        &bolted,
        profile,
        run.profile.mode == ProfileMode::Nl,
    );
    log_run(run_dir, run, "optimize", &cmd);
    let rec = run_command(&cmd, run_dir, timeout(), &[], Some(&bolted));
    let ok = rec.exit_code == 0 && bolted.is_file();
    record(run, rec);
    if ok {
        let detail = match run.profile.mode {
            ProfileMode::Instrument => {
                "created BOLT candidate from instrumentation fdata (original binary, not the instrumented copy)"
            }
            ProfileMode::Lbr => "created BOLT candidate from LBR profile",
            ProfileMode::Nl => "created BOLT candidate from no-LBR samples",
            ProfileMode::Unavailable => "created BOLT candidate",
        };
        run.transforms_applied.push(Transform {
            tool: "llvm-bolt".into(),
            status: "applied".into(),
            detail: detail.into(),
        });
        *current = bolted;
    } else {
        run.transforms_failed.push(Transform {
            tool: "llvm-bolt".into(),
            status: "failed".into(),
            detail: "llvm-bolt ran but did not produce a candidate".into(),
        });
    }
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
            for script in ["test.sh", "bench.sh", "workload.sh"] {
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
        std::env::set_var("QUENCH_BENCH_REPETITIONS", "3");
        std::env::set_var("QUENCH_BENCH_WARMUP", "0");
        std::env::remove_var("QUENCH_FORCE_UNAVAILABLE");
        std::env::remove_var("QUENCH_FORCE_PROFILE_MODE");
        std::env::remove_var("QUENCH_FORCE_LBR");
        std::env::remove_var("QUENCH_BOLT_RT_INSTR");
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
        assert_eq!(c.spread_ms, 30.0);
        assert_eq!(c.min_ms, 100.0);
        assert_eq!(c.max_ms, 130.0);
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
        let warning = run.report["stabilityWarning"].as_str().unwrap_or("");
        assert!(warning.contains("does not change"), "warning={warning}");
        assert_eq!(run.report["keptCandidate"], false);
    }

    #[test]
    fn warmup_is_excluded_and_repetitions_are_configurable() {
        let (root, _home, _guard) = setup_fixture("success");
        std::env::set_var("QUENCH_BENCH_REPETITIONS", "4");
        std::env::set_var("QUENCH_BENCH_WARMUP", "2");
        let cfg = load_config(&root.join("quench.yaml")).unwrap();
        let run = optimize(&cfg).expect("optimize");
        assert_core_native_report(&run);
        assert_eq!(run.report["benchmarkRepetitions"], 4);
        assert_eq!(run.report["benchmarkWarmup"], 2);
        assert_eq!(
            run.report["benchmarkCommand"].as_str(),
            cfg.benchmark.as_deref()
        );
        let logs = run.logs.join("\n");
        assert!(logs.contains("baseline warmup 1/2 (excluded)"), "{logs}");
        assert!(logs.contains("baseline repetition 4/4"), "{logs}");
        assert!(logs.contains("candidate repetition 4/4"), "{logs}");
        let samples = run.report["runtime"]["baseline"]["samples_ms"]
            .as_array()
            .expect("baseline samples");
        assert_eq!(samples.len(), 4);
        let bench_cmds: Vec<_> = run
            .commands
            .iter()
            .filter(|c| c.command.contains("bench.sh"))
            .collect();
        assert_eq!(bench_cmds.len(), 12, "2 warmup + 4 measured, twice");
        assert!(bench_cmds
            .iter()
            .all(|c| c.command == bench_cmds[0].command));
        assert_eq!(run.report["benchmarkedInstrumented"], false);
        assert_eq!(run.report["minImprovementPercent"], 1.0);
        assert_eq!(run.report["maxRegressionPercent"], 2.0);
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
        assert!(r.get("profileMode").and_then(|v| v.as_str()).is_some());
        assert_eq!(
            r.get("benchmarkedInstrumented").and_then(|v| v.as_bool()),
            Some(false)
        );
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

    #[test]
    fn profile_mode_unavailable_is_recorded() {
        let (root, _home, _guard) = setup_fixture("success");
        std::env::set_var("QUENCH_FORCE_PROFILE_MODE", "unavailable");
        std::env::set_var("QUENCH_FORCE_UNAVAILABLE", "llvm-bolt,perf");
        let cfg = load_config(&root.join("quench.yaml")).unwrap();
        let run = optimize(&cfg).expect("optimize");
        assert_core_native_report(&run);
        assert_eq!(run.report["profileMode"], "unavailable");
        assert_eq!(run.report["benchmarkedInstrumented"], false);
        assert_eq!(run.report["benchmarkedOriginal"], true);
    }

    #[test]
    fn lbr_forced_path_records_lbr_commands() {
        let (root, _home, _guard) = setup_fixture("success");
        std::env::set_var("QUENCH_FORCE_PROFILE_MODE", "lbr");
        let cfg = load_config(&root.join("quench.yaml")).unwrap();
        let run = optimize(&cfg).expect("optimize");
        assert_core_native_report(&run);
        assert_eq!(run.report["profileMode"], "lbr");
        assert_eq!(
            run.report["lbrProbeCommand"],
            "perf record -e cycles:u -j any,u -- sleep 0.3"
        );
        if which("perf").is_some() {
            assert!(
                run.commands
                    .iter()
                    .any(|c| c.command.contains("cycles:u -j any,u")),
                "LBR path must record the branch-stack perf command"
            );
        }
        assert_eq!(run.report["benchmarkedInstrumented"], false);
    }

    #[test]
    fn missing_libbolt_rt_does_not_instrument() {
        let (root, _home, _guard) = setup_fixture("success");
        std::env::set_var("QUENCH_FORCE_PROFILE_MODE", "instrument");
        std::env::set_var("QUENCH_BOLT_RT_INSTR", "/tmp/quench-missing-libbolt-rt.a");
        let cfg = load_config(&root.join("quench.yaml")).unwrap();
        let run = optimize(&cfg).expect("optimize");
        assert_core_native_report(&run);
        assert_ne!(run.report["profileMode"], "instrument");
        assert!(
            run.transforms_failed
                .iter()
                .any(|t| t.tool == "llvm-bolt"
                    && (t.status == "Unavailable" || t.status == "skipped"))
        );
        assert!(!run
            .commands
            .iter()
            .any(|c| c.command.contains("-instrument ")));
    }

    #[test]
    fn missing_relocs_skip_instrumentation() {
        let (root, _home, _guard) = setup_fixture("success");
        if which("llvm-bolt").is_none() {
            return;
        }
        std::env::set_var("QUENCH_FORCE_PROFILE_MODE", "instrument");
        std::env::set_var("QUENCH_FORCE_LBR", "0");
        let cfg = load_config(&root.join("quench.yaml")).unwrap();
        let run = optimize(&cfg).expect("optimize");
        assert_core_native_report(&run);
        assert_eq!(run.report["profileMode"], "instrument");
        assert_eq!(run.report["hasTextRelocs"], false);
        assert!(run.transforms_failed.iter().any(|t| t.tool == "llvm-bolt"
            && t.status == "skipped"
            && t.detail.contains("rela.text")));
        assert!(!run
            .commands
            .iter()
            .any(|c| c.command.contains("-instrument ")));
        assert_eq!(run.report["benchmarkedInstrumented"], false);
    }

    #[test]
    fn instrument_fallback_rejects_without_inventing_keep() {
        if which("llvm-bolt").is_none() || which("gcc").is_none() {
            return;
        }
        let (root, _home, _guard) = setup_fixture("bolt-fallback");
        std::env::set_var("QUENCH_FORCE_LBR", "0");
        let cfg = load_config(&root.join("quench.yaml")).unwrap();
        let run = optimize(&cfg).expect("optimize");
        assert_core_native_report(&run);
        assert_eq!(run.report["profileMode"], "instrument");
        assert_eq!(run.report["keptCandidate"], false);
        assert_eq!(run.report["benchmarkedInstrumented"], false);
        assert_eq!(run.report["benchmarkedOriginal"], true);
        let bench = cfg.benchmark.clone().unwrap();
        for rec in &run.commands {
            let inst = rec
                .artifact_path
                .as_deref()
                .unwrap_or("")
                .contains("instrumented");
            if rec.command.contains(&bench) || rec.command.contains("bench.sh") {
                assert!(
                    !inst,
                    "benchmark ran against instrumented binary: {}",
                    rec.command
                );
            }
            if inst {
                assert!(
                    rec.command.contains("-instrument")
                        || rec.command.contains("workload")
                        || rec.command.contains("./app")
                        || rec.command.contains("instrumented"),
                    "instrumented artifact used unexpectedly: {}",
                    rec.command
                );
            }
        }
        let reason = run.report["reason"].as_str().unwrap_or("");
        assert!(
            reason.contains("min_improvement")
                || reason.contains("No candidate")
                || reason.contains("regression")
                || reason.contains("failed"),
            "reason={reason}"
        );
        assert_ne!(run.status, RunStatus::Complete);
    }

    #[test]
    fn invalid_elf_is_not_optimized() {
        let (root, _home, _guard) = setup_fixture("success");
        std::fs::write(root.join("app"), b"not-an-elf").unwrap();
        let mut cfg = load_config(&root.join("quench.yaml")).unwrap();
        cfg.build = Some("true".into());
        let run = optimize(&cfg).expect("optimize returns a report");
        assert_eq!(run.report["keptCandidate"], false);
        assert_eq!(run.status, RunStatus::Failed);
        let err = run.report["error"].as_str().unwrap_or("");
        assert!(
            err.contains("Unsupported format") || err.to_lowercase().contains("elf"),
            "error={err}"
        );
    }

    #[test]
    fn missing_benchmark_script_fails_without_inventing_metrics() {
        let (root, _home, _guard) = setup_fixture("missing-script");
        let cfg = load_config(&root.join("quench.yaml")).unwrap();
        let run = optimize(&cfg).expect("optimize should return a report");
        assert_eq!(run.status, RunStatus::Failed);
        assert_eq!(run.report["ok"], false);
        assert_eq!(run.report["keptCandidate"], false);
        assert!(run.report.get("medianMs").is_none() || run.report["medianMs"].is_null());
        let err = run.report["error"].as_str().unwrap_or("");
        assert!(
            err.contains("preflight") || err.contains("nope.sh"),
            "error={err}"
        );
    }
}
