use crate::pipeline::{load_run, PipelineRun, RunStatus};
use serde_json::json;

pub fn report_text(run: &PipelineRun) -> String {
    let r = &run.report;
    let kept = r
        .get("keptCandidate")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mut lines = vec![
        format!("quench-agent report {}", run.run_id),
        format!("status           {:?}", run.status),
        format!("project          {}", run.project),
        format!("kind             {}", run.kind),
        format!("input path       {}", run.input_path),
        format!("config           {}", run.config_path),
        format!(
            "baseline identity {}",
            r.get("baselineSha256")
                .and_then(|v| v.as_str())
                .unwrap_or("-")
        ),
        format!(
            "candidate identity {}",
            r.get("candidateSha256")
                .and_then(|v| v.as_str())
                .unwrap_or("-")
        ),
        format!(
            "build command    {}",
            r.get("buildCommand")
                .and_then(|v| v.as_str())
                .unwrap_or("-")
        ),
        format!(
            "test command     {}",
            r.get("testCommand").and_then(|v| v.as_str()).unwrap_or("-")
        ),
        format!(
            "profile command  {}",
            r.get("profileCommand")
                .and_then(|v| v.as_str())
                .unwrap_or("-")
        ),
        format!(
            "benchmark        {}",
            r.get("benchmarkCommand")
                .and_then(|v| v.as_str())
                .unwrap_or("-")
        ),
    ];
    if let Some(size) = r.get("artifactSize") {
        lines.push(format!(
            "artifact size    baseline={} candidate={}",
            size.get("baselineBytes").unwrap_or(&json!(null)),
            size.get("candidateBytes").unwrap_or(&json!(null))
        ));
    }
    lines.push(format!(
        "test result      {}",
        r.get("testResult").and_then(|v| v.as_str()).unwrap_or("-")
    ));
    if let Some(med) = r.get("medianMs") {
        lines.push(format!("median ms        {med}"));
    }
    if let Some(p95) = r.get("p95Ms") {
        lines.push(format!("p95 ms           {p95}"));
    }
    if let Some(impr) = r.get("medianImprovementPercent").and_then(|v| v.as_f64()) {
        lines.push(format!("median improvement {impr:.3}%"));
    }
    lines.push(format!(
        "min_improvement_percent {}",
        r.get("minImprovementPercent")
            .and_then(|v| v.as_f64())
            .map(|n| n.to_string())
            .unwrap_or_else(|| "-".into())
    ));
    lines.push(format!(
        "max_regression_percent  {}",
        r.get("maxRegressionPercent")
            .and_then(|v| v.as_f64())
            .map(|n| n.to_string())
            .unwrap_or_else(|| "-".into())
    ));
    lines.push(format!(
        "profile mode     {}",
        r.get("profileMode").and_then(|v| v.as_str()).unwrap_or("-")
    ));
    if let Some(reason) = r.get("profileReason").and_then(|v| v.as_str()) {
        if !reason.is_empty() {
            lines.push(format!("profile reason   {reason}"));
        }
    }
    lines.push(format!(
        "LBR probe        {} ({})",
        r.get("lbrProbeCommand")
            .and_then(|v| v.as_str())
            .unwrap_or("perf record -e cycles:u -j any,u -- sleep 0.3"),
        match r.get("lbrProbeOk").and_then(|v| v.as_bool()) {
            Some(true) => "ok",
            Some(false) => "failed",
            None => "not run",
        }
    ));
    if let Some(detail) = r.get("lbrProbeDetail").and_then(|v| v.as_str()) {
        if !detail.is_empty() {
            lines.push(format!("LBR probe detail {detail}"));
        }
    }
    lines.push(format!(
        "bench artifacts  original={} candidate={} instrumented={}",
        r.get("benchmarkedOriginal")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        r.get("benchmarkedCandidate")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        r.get("benchmarkedInstrumented")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    ));
    if let Some(w) = r.get("profileWarning").and_then(|v| v.as_str()) {
        if !w.is_empty() {
            lines.push(format!("profile warning  {w}"));
        }
    }
    lines.push(format!(
        "candidate        {}",
        if kept {
            "kept"
        } else {
            "rejected / not produced"
        }
    ));
    if let Some(reason) = r.get("reason").and_then(|v| v.as_str()) {
        lines.push(format!("decision         {reason}"));
    }
    lines.push(format!(
        "repetitions      {}",
        r.get("benchmarkRepetitions")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
    ));
    lines.push("tools:".into());
    if let Some(obj) = r.get("toolVersions").and_then(|v| v.as_object()) {
        for (id, info) in obj {
            let status = info.get("status").and_then(|v| v.as_str()).unwrap_or("-");
            lines.push(format!("  {id}: {status}"));
        }
    }
    lines.push("transforms applied:".into());
    if run.transforms_applied.is_empty() {
        lines.push("  (none)".into());
    }
    for t in &run.transforms_applied {
        lines.push(format!("  {} [{}] {}", t.tool, t.status, t.detail));
    }
    lines.push("transforms failed / unavailable:".into());
    if run.transforms_failed.is_empty() {
        lines.push("  (none)".into());
    }
    for t in &run.transforms_failed {
        lines.push(format!("  {} [{}] {}", t.tool, t.status, t.detail));
    }
    lines.push(format!(
        "reproducible     {}",
        r.get("reproducibleCommand")
            .and_then(|v| v.as_str())
            .unwrap_or("quench-agent optimize --config quench.yaml")
    ));
    match run.status {
        RunStatus::Complete => {}
        RunStatus::VerificationFailed => {
            lines.push("NOTE: candidate was discarded after test failure.".into())
        }
        RunStatus::Failed => lines.push("NOTE: candidate was not kept.".into()),
        RunStatus::Running => lines.push("NOTE: run still in progress.".into()),
    }
    lines.push("This is a local optimization report, not a certificate.".into());
    lines.join("\n") + "\n"
}

pub fn print_report(run_id: &str) -> Result<PipelineRun, String> {
    load_run(run_id)
}
