use std::path::{Path, PathBuf};

pub const DEFAULT_SAMPLE_REL: &str = "samples/match-engine/quench.yaml";
pub const ERR_ORIGIN: &str = "origin not allowed";
pub const ERR_CONFIG_PATH: &str =
    "configPath must be a file inside the allowed workspace root; absolute paths outside the workspace and `..` escapes are rejected";
pub const ERR_INSPECT_PATH: &str =
    "inspect path must be a file inside the allowed workspace root; arbitrary absolute paths are rejected";
pub const ERR_HOST: &str = "host not allowed; quench-agent only serves loopback";

fn default_origins() -> Vec<String> {
    vec![
        "http://127.0.0.1:8080".into(),
        "http://localhost:8080".into(),
        "http://127.0.0.1:8081".into(),
        "http://localhost:8081".into(),
        "http://127.0.0.1:4783".into(),
        "http://localhost:4783".into(),
    ]
}

pub fn allowed_origins() -> Vec<String> {
    let mut origins = default_origins();
    if let Ok(extra) = std::env::var("QUENCH_ALLOWED_ORIGINS") {
        for part in extra.split(',') {
            let trimmed = part.trim();
            if !trimmed.is_empty() {
                origins.push(trimmed.to_string());
            }
        }
    }
    origins
}

pub fn origin_allowed(origin: &str) -> bool {
    let origin = origin.trim();
    if origin.is_empty() || origin.eq_ignore_ascii_case("null") {
        return false;
    }
    allowed_origins()
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(origin))
}

/// Browser POSTs always send Origin. Unknown origins are rejected.
/// A missing Origin is allowed for the local CLI and the unix-socket proxy hop.
pub fn mutating_origin_ok(origin: Option<&str>) -> Result<(), String> {
    match origin.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(()),
        Some(value) if origin_allowed(value) => Ok(()),
        Some(_) => Err(ERR_ORIGIN.into()),
    }
}

pub fn cors_header_lines(origin: Option<&str>) -> String {
    match origin.map(str::trim).filter(|s| !s.is_empty()) {
        Some(value) if origin_allowed(value) => format!(
            "Access-Control-Allow-Origin: {value}\r\nAccess-Control-Allow-Headers: Content-Type, X-Filename, X-Quench-Path\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nVary: Origin\r\n"
        ),
        _ => "Vary: Origin\r\n".into(),
    }
}

pub fn host_allowed(host: Option<&str>) -> bool {
    let Some(raw) = host.map(str::trim).filter(|s| !s.is_empty()) else {
        return true;
    };
    let without_brackets = raw.trim_start_matches('[');
    let hostname = without_brackets
        .split([':', ']'])
        .find(|part| !part.is_empty() && *part != "::1")
        .unwrap_or(without_brackets);
    let hostname = hostname.trim().to_ascii_lowercase();
    if raw.to_ascii_lowercase().contains("::1") {
        return true;
    }
    matches!(
        hostname.as_str(),
        "127.0.0.1" | "localhost" | "localhost.localdomain"
    )
}

pub fn bind_is_loopback(bind: &str) -> bool {
    let trimmed = bind.trim();
    trimmed.starts_with("127.0.0.1:")
        || trimmed.starts_with("localhost:")
        || trimmed.starts_with("[::1]:")
        || trimmed == "127.0.0.1"
        || trimmed == "localhost"
        || trimmed == "[::1]"
}

fn looks_like_workspace(dir: &Path) -> bool {
    dir.join("samples/match-engine/quench.yaml").is_file()
        || dir.join("quench.yaml.example").is_file()
        || dir.join("agent/Cargo.toml").is_file()
}

fn discover_workspace() -> Option<PathBuf> {
    if let Ok(cwd) = std::env::current_dir() {
        let mut cur = Some(cwd.as_path());
        while let Some(dir) = cur {
            if looks_like_workspace(dir) {
                return Some(dir.to_path_buf());
            }
            cur = dir.parent();
        }
    }
    let fallback = PathBuf::from("/workspace");
    if looks_like_workspace(&fallback) {
        return Some(fallback);
    }
    None
}

pub fn workspace_root() -> PathBuf {
    if let Ok(value) = std::env::var("QUENCH_WORKSPACE") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    discover_workspace()
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn default_sample_rel() -> String {
    std::env::var("QUENCH_SAMPLE_CONFIG")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_SAMPLE_REL.to_string())
}

fn absolute_root(root: &Path) -> PathBuf {
    if let Ok(canon) = std::fs::canonicalize(root) {
        return canon;
    }
    if root.is_absolute() {
        root.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(root)
    }
}

fn is_within(root: &Path, candidate: &Path) -> bool {
    candidate.starts_with(root)
}

fn resolve_path(raw: &str, err: &str) -> Result<PathBuf, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(err.to_string());
    }
    if raw.contains('\0') || raw.len() > 4096 {
        return Err(err.to_string());
    }
    let root = absolute_root(&workspace_root());
    let candidate = Path::new(raw);
    let joined = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        root.join(candidate)
    };
    let canon = std::fs::canonicalize(&joined).map_err(|_| err.to_string())?;
    if !is_within(&root, &canon) {
        return Err(err.to_string());
    }
    if !canon.is_file() {
        return Err(err.to_string());
    }
    Ok(canon)
}

pub fn resolve_config_path(raw: &str) -> Result<PathBuf, String> {
    resolve_path(raw, ERR_CONFIG_PATH)
}

pub fn resolve_inspect_path(raw: &str) -> Result<PathBuf, String> {
    resolve_path(raw, ERR_INSPECT_PATH)
}

pub fn sample_config_path() -> Option<PathBuf> {
    resolve_config_path(&default_sample_rel()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_workspace<T>(name: &str, f: impl FnOnce(&Path) -> T) -> T {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let root = std::env::temp_dir().join(format!(
            "quench-sec-{}-{}-{}",
            name,
            std::process::id(),
            crate::util::now_ms()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("samples/match-engine")).unwrap();
        std::fs::write(
            root.join("samples/match-engine/quench.yaml"),
            "project: match-engine\nkind: elf\nbinary: ./app\nbuild: true\ntest: true\nbenchmark: true\n",
        )
        .unwrap();
        std::env::set_var("QUENCH_WORKSPACE", &root);
        std::env::remove_var("QUENCH_SAMPLE_CONFIG");
        std::env::remove_var("QUENCH_ALLOWED_ORIGINS");
        let out = f(&root);
        let _ = std::fs::remove_dir_all(&root);
        std::env::remove_var("QUENCH_WORKSPACE");
        out
    }

    #[test]
    fn unknown_origin_is_rejected() {
        assert!(mutating_origin_ok(Some("https://evil.example")).is_err());
        assert!(mutating_origin_ok(Some("https://evil.example/")).is_err());
        assert!(mutating_origin_ok(Some("null")).is_err());
        assert!(!origin_allowed("https://attacker.test"));
    }

    #[test]
    fn local_dev_origin_is_allowed() {
        assert!(mutating_origin_ok(Some("http://127.0.0.1:8080")).is_ok());
        assert!(mutating_origin_ok(Some("http://localhost:8080")).is_ok());
        assert!(mutating_origin_ok(None).is_ok());
        assert!(origin_allowed("http://127.0.0.1:8081"));
    }

    #[test]
    fn cors_never_uses_star() {
        let denied = cors_header_lines(Some("https://evil.example"));
        assert!(!denied.contains('*'));
        assert!(!denied
            .to_ascii_lowercase()
            .contains("access-control-allow-origin"));
        let allowed = cors_header_lines(Some("http://127.0.0.1:8080"));
        assert!(allowed.contains("Access-Control-Allow-Origin: http://127.0.0.1:8080"));
        assert!(!allowed.contains('*'));
    }

    #[test]
    fn out_of_root_config_path_is_rejected() {
        with_workspace("escape", |root| {
            assert!(resolve_config_path("/etc/passwd").is_err());
            assert!(resolve_config_path("/tmp/quench.yaml").is_err());
            assert!(resolve_config_path("../quench.yaml").is_err());
            assert!(resolve_config_path("../../etc/passwd").is_err());
            let abs_outside = std::env::temp_dir().join("quench-outside.yaml");
            std::fs::write(&abs_outside, "project: x\nkind: elf\n").unwrap();
            assert!(resolve_config_path(&abs_outside.to_string_lossy()).is_err());
            let _ = std::fs::remove_file(&abs_outside);

            let ok_rel = resolve_config_path("samples/match-engine/quench.yaml").unwrap();
            assert!(ok_rel.starts_with(root));
            let ok_abs = resolve_config_path(
                &root
                    .join("samples/match-engine/quench.yaml")
                    .to_string_lossy(),
            )
            .unwrap();
            assert!(ok_abs.starts_with(root));
        });
    }

    #[test]
    fn inspect_path_rejects_arbitrary_absolute() {
        with_workspace("inspect", |root| {
            assert!(resolve_inspect_path("/etc/hosts").is_err());
            let ok = resolve_inspect_path("samples/match-engine/quench.yaml").unwrap();
            assert!(ok.starts_with(root));
        });
    }

    #[test]
    fn extra_origins_from_env() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::set_var("QUENCH_ALLOWED_ORIGINS", "https://studio.local:8080");
        assert!(origin_allowed("https://studio.local:8080"));
        std::env::remove_var("QUENCH_ALLOWED_ORIGINS");
    }

    #[test]
    fn bind_rejects_non_loopback() {
        assert!(bind_is_loopback("127.0.0.1:4783"));
        assert!(bind_is_loopback("localhost:4783"));
        assert!(!bind_is_loopback("0.0.0.0:4783"));
        assert!(!bind_is_loopback("[::]:4783"));
    }

    #[test]
    fn host_header_must_be_loopback() {
        assert!(host_allowed(Some("127.0.0.1:4783")));
        assert!(host_allowed(Some("localhost:4783")));
        assert!(!host_allowed(Some("evil.example")));
        assert!(!host_allowed(Some("8.8.8.8:4783")));
    }
}
