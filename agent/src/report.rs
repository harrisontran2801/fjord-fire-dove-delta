use crate::pipeline::{load_run, PipelineRun, RunStatus};
use serde_json::json;

pub fn report_text(run: &PipelineRun) -> String {
    let r = &run.report;
    let mut lines = vec![
        format!("quench-agent report {}", run.run_id),
        format!("status           {:?}", run.status),
        format!("project          {}", run.project),
        format!("input path       {}", run.input_path),
        format!("config           {}", run.config_path),
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
            "benchmark        {}",
            r.get("benchmarkCommand")
                .and_then(|v| v.as_str())
                .unwrap_or("-")
        ),
        format!(
            "baseline sha256  {}",
            r.get("baselineSha256")
                .and_then(|v| v.as_str())
                .unwrap_or("-")
        ),
        format!(
            "candidate sha256 {}",
            r.get("candidateSha256")
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
    lines.push(format!(
        "repetitions      {}",
        r.get("benchmarkRepetitions")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
    ));
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
