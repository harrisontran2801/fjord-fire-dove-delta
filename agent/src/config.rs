use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProjectKind {
    Elf,
    Docker,
    Oci,
}

impl ProjectKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProjectKind::Elf => "elf",
            ProjectKind::Docker => "docker",
            ProjectKind::Oci => "oci",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct QuenchConfig {
    pub project: String,
    pub kind: ProjectKind,
    pub binary: Option<String>,
    pub build: Option<String>,
    pub test: Option<String>,
    pub benchmark: Option<String>,
    pub profile: Option<String>,
    pub dockerfile: Option<String>,
    pub image: Option<String>,
    pub max_regression_percent: f64,
    pub min_improvement_percent: f64,
    pub source_path: PathBuf,
    pub project_root: PathBuf,
}

fn strip_quotes(s: &str) -> String {
    let t = s.trim();
    if (t.starts_with('"') && t.ends_with('"') && t.len() >= 2)
        || (t.starts_with('\'') && t.ends_with('\'') && t.len() >= 2)
    {
        t[1..t.len() - 1].to_string()
    } else {
        t.to_string()
    }
}

fn parse_simple_yaml(text: &str) -> Result<std::collections::BTreeMap<String, String>, String> {
    let mut map = std::collections::BTreeMap::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once(':') else {
            return Err(format!("invalid YAML at line {}: {raw}", i + 1));
        };
        let key = k.trim().to_string();
        if key.is_empty() {
            continue;
        }
        map.insert(key, strip_quotes(v));
    }
    Ok(map)
}

fn required(map: &std::collections::BTreeMap<String, String>, key: &str) -> Result<String, String> {
    map.get(key)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("quench.yaml missing required field `{key}`"))
}

fn opt(map: &std::collections::BTreeMap<String, String>, key: &str) -> Option<String> {
    map.get(key)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn parse_f64(
    map: &std::collections::BTreeMap<String, String>,
    key: &str,
    default: f64,
) -> Result<f64, String> {
    match map.get(key) {
        None => Ok(default),
        Some(s) => s
            .trim()
            .parse::<f64>()
            .map_err(|_| format!("invalid number for `{key}`: {s}")),
    }
}

pub fn parse_config_text(text: &str, source_path: &Path) -> Result<QuenchConfig, String> {
    let map = parse_simple_yaml(text)?;
    let project = required(&map, "project")?;
    let kind = match required(&map, "kind")?.to_lowercase().as_str() {
        "elf" => ProjectKind::Elf,
        "docker" => ProjectKind::Docker,
        "oci" => ProjectKind::Oci,
        other => {
            return Err(format!(
                "unsupported kind `{other}` (supported: elf, docker, oci)"
            ))
        }
    };
    let project_root = source_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    Ok(QuenchConfig {
        project,
        kind,
        binary: opt(&map, "binary"),
        build: opt(&map, "build"),
        test: opt(&map, "test"),
        benchmark: opt(&map, "benchmark"),
        profile: opt(&map, "profile"),
        dockerfile: opt(&map, "dockerfile"),
        image: opt(&map, "image"),
        max_regression_percent: parse_f64(&map, "max_regression_percent", 2.0)?,
        min_improvement_percent: parse_f64(&map, "min_improvement_percent", 1.0)?,
        source_path: source_path.to_path_buf(),
        project_root,
    })
}

pub fn load_config(path: &Path) -> Result<QuenchConfig, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("could not read {}: {e}", path.display()))?;
    parse_config_text(&text, path)
}

pub fn validate_for_optimize(cfg: &QuenchConfig) -> Result<(), String> {
    match cfg.kind {
        ProjectKind::Elf => {
            if cfg.binary.is_none() {
                return Err("ELF optimize requires `binary` in quench.yaml".into());
            }
            if cfg.build.as_deref().unwrap_or("").trim().is_empty() {
                return Err("Missing input: `build` command is required. Native optimize will not invent a build.".into());
            }
            if cfg.test.as_deref().unwrap_or("").trim().is_empty() {
                return Err("Missing input: `test` command is required. Native optimize will not claim tests passed without running them.".into());
            }
            if cfg.benchmark.as_deref().unwrap_or("").trim().is_empty() {
                return Err("Missing input: `benchmark` command is required.".into());
            }
            Ok(())
        }
        ProjectKind::Docker | ProjectKind::Oci => {
            // Rewrite is not implemented. Optimize records an inspect/recommendation
            // report even when build/test commands are missing.
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_example() {
        let text = r#"
project: match-engine
kind: elf
binary: ./target/release/match-engine
build: cargo build --release
test: ./scripts/test.sh
benchmark: ./scripts/bench.sh
profile: ./scripts/workload.sh
max_regression_percent: 2
min_improvement_percent: 1
"#;
        let cfg = parse_config_text(text, Path::new("/tmp/quench.yaml")).unwrap();
        assert_eq!(cfg.project, "match-engine");
        assert_eq!(cfg.kind, ProjectKind::Elf);
        assert_eq!(cfg.binary.as_deref(), Some("./target/release/match-engine"));
        assert_eq!(cfg.max_regression_percent, 2.0);
    }
}
