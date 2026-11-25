use globset::{Glob, GlobSet, GlobSetBuilder};
use std::fs;
use std::path::Path;

use crate::error::{CoreError, CoreResult};
use crate::security::TEMP_FILE_PREFIX;

/// Immutable security rules that are ALWAYS excluded and cannot be overridden.
pub const IMMUTABLE_SECURITY_EXCLUDES: &[&str] = &[
    ".git",
    ".git/**",
    "**/.git",
    "**/.git/**",
    ".env",
    ".env.*",
    "**/.env",
    "**/.env.*",
    "*.pem",
    "**/*.pem",
    "*.key",
    "**/*.key",
    "*.p12",
    "**/*.p12",
    "*.pfx",
    "**/*.pfx",
    "*.jks",
    "**/*.jks",
    "*.keystore",
    "**/*.keystore",
    "*.kdbx",
    "**/*.kdbx",
    "id_rsa",
    "**/id_rsa",
    "id_ed25519",
    "**/id_ed25519",
    "*credentials*.json",
    "**/*credentials*.json",
    "secrets.yml",
    "**/secrets.yml",
    "secrets.yaml",
    "**/secrets.yaml",
    ".npmrc",
    "**/.npmrc",
    ".pypirc",
    "**/.pypirc",
    ".netrc",
    "**/.netrc",
];

/// Default dependency cache, generated build output, and OS junk directories/files.
pub const DEFAULT_CACHE_AND_BUILD_EXCLUDES: &[&str] = &[
    "node_modules",
    "node_modules/**",
    "**/node_modules",
    "**/node_modules/**",
    "target",
    "target/**",
    "**/target",
    "**/target/**",
    "dist",
    "dist/**",
    "**/dist",
    "**/dist/**",
    "build",
    "build/**",
    "**/build",
    "**/build/**",
    ".next",
    ".next/**",
    "**/.next/**",
    ".nuxt",
    ".nuxt/**",
    "**/.nuxt/**",
    ".svelte-kit",
    ".svelte-kit/**",
    "**/.svelte-kit/**",
    ".turbo",
    ".turbo/**",
    "**/.turbo/**",
    ".cache",
    ".cache/**",
    "**/.cache/**",
    ".parcel-cache",
    ".parcel-cache/**",
    "__pycache__",
    "__pycache__/**",
    "**/__pycache__/**",
    ".venv",
    ".venv/**",
    "venv",
    "venv/**",
    ".pytest_cache",
    ".pytest_cache/**",
    ".mypy_cache",
    ".mypy_cache/**",
    ".ruff_cache",
    ".ruff_cache/**",
    ".gradle",
    ".gradle/**",
    "vendor",
    "vendor/**",
    "bin/Debug/**",
    "bin/Release/**",
    "obj/**",
    "out/**",
    ".idea/**",
    ".vs/**",
    "*.log",
    "**/*.log",
    "*.tmp",
    "**/*.tmp",
    "*.swp",
    "**/*.swp",
    "*.swo",
    "**/*.swo",
    ".DS_Store",
    "**/.DS_Store",
    "Thumbs.db",
    "**/Thumbs.db",
    "desktop.ini",
    "**/desktop.ini",
];

#[derive(Debug, Clone)]
pub struct IgnoreMatcher {
    immutable: GlobSet,
    defaults: GlobSet,
    custom: GlobSet,
    max_file_size_bytes: u64,
}

impl IgnoreMatcher {
    pub fn build(
        project_root: &Path,
        global_excludes: &[String],
        project_excludes: &[String],
        respect_gitignore: bool,
        max_file_size_mb: u64,
    ) -> CoreResult<Self> {
        let immutable = compile_globs(IMMUTABLE_SECURITY_EXCLUDES.iter().copied())?;
        let defaults = compile_globs(DEFAULT_CACHE_AND_BUILD_EXCLUDES.iter().copied())?;

        let mut custom_patterns: Vec<String> = Vec::new();
        custom_patterns.extend(global_excludes.iter().cloned());
        custom_patterns.extend(project_excludes.iter().cloned());

        // Load optional .agentrestoreignore in project root
        let ignore_file = project_root.join(".agentrestoreignore");
        if ignore_file.is_file() {
            if let Ok(content) = fs::read_to_string(&ignore_file) {
                custom_patterns.extend(parse_ignore_lines(&content));
            }
        }

        // Optionally honor .gitignore patterns (note: untracked non-ignored files are STILL backed up!)
        if respect_gitignore {
            let gitignore_file = project_root.join(".gitignore");
            if gitignore_file.is_file() {
                if let Ok(content) = fs::read_to_string(&gitignore_file) {
                    custom_patterns.extend(parse_ignore_lines(&content));
                }
            }
        }

        let custom = compile_globs(custom_patterns.iter().map(|s| s.as_str()))?;

        Ok(Self {
            immutable,
            defaults,
            custom,
            max_file_size_bytes: max_file_size_mb.saturating_mul(1024 * 1024),
        })
    }

    pub fn validate_patterns(patterns: &[String]) -> CoreResult<()> {
        for p in patterns {
            let trimmed = p.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            Glob::new(trimmed).map_err(|e| CoreError::InvalidPattern {
                pattern: trimmed.to_string(),
                reason: e.to_string(),
            })?;
        }
        Ok(())
    }

    /// Returns true if a normalized forward-slash relative path should be excluded.
    pub fn is_excluded(&self, rel_path: &str, is_dir: bool) -> bool {
        let normalized = rel_path.trim_start_matches("./").trim_end_matches('/');
        if normalized.is_empty() {
            return false;
        }

        // Hard-protect any .git segment or temp staging file
        for seg in normalized.split('/') {
            if seg.eq_ignore_ascii_case(".git") || seg.starts_with(TEMP_FILE_PREFIX) {
                return true;
            }
        }

        // Keep safe template files like .env.example or .env.sample eligible?
        // Security-first: default excludes all .env* EXCEPT explicit .env.example / .env.sample / .env.template
        let file_name = normalized.rsplit('/').next().unwrap_or(normalized);
        let is_safe_env_template = matches!(
            file_name,
            ".env.example" | ".env.sample" | ".env.template"
        );

        if !is_safe_env_template && self.immutable.is_match(normalized) {
            return true;
        }
        if self.defaults.is_match(normalized) || self.custom.is_match(normalized) {
            return true;
        }

        if is_dir {
            let dir_probe = format!("{}/", normalized);
            if self.defaults.is_match(&dir_probe) || self.custom.is_match(&dir_probe) {
                return true;
            }
        }

        false
    }

    pub fn exceeds_max_file_size(&self, size_bytes: u64) -> bool {
        self.max_file_size_bytes > 0 && size_bytes > self.max_file_size_bytes
    }
}

fn compile_globs<'a, I: IntoIterator<Item = &'a str>>(patterns: I) -> CoreResult<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for raw in patterns {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        for expanded in expand_gitignore_style(trimmed) {
            let glob = Glob::new(&expanded).map_err(|e| CoreError::InvalidPattern {
                pattern: trimmed.to_string(),
                reason: e.to_string(),
            })?;
            builder.add(glob);
        }
    }
    builder.build().map_err(|e| CoreError::InvalidPattern {
        pattern: "<globset>".to_string(),
        reason: e.to_string(),
    })
}

fn parse_ignore_lines(content: &str) -> Vec<String> {
    content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('!'))
        .map(|l| l.to_string())
        .collect()
}

/// Converts simple gitignore-style patterns (e.g. `dist/`, `*.bak`, `/root_only`) into globset patterns.
fn expand_gitignore_style(pattern: &str) -> Vec<String> {
    let mut p = pattern.replace('\\', "/");
    let anchored = p.starts_with('/');
    if anchored {
        p = p.trim_start_matches('/').to_string();
    }
    let dir_only = p.ends_with('/');
    if dir_only {
        p = p.trim_end_matches('/').to_string();
    }
    if p.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();
    if anchored || p.contains('/') {
        out.push(p.clone());
        out.push(format!("{}/**", p));
    } else {
        out.push(p.clone());
        out.push(format!("**/{}", p));
        out.push(format!("{}/**", p));
        out.push(format!("**/{}/**", p));
    }
    out
}

pub fn default_exclusion_summary() -> (Vec<String>, Vec<String>) {
    let security = vec![
        ".git/** (immutable)".into(),
        ".env, .env.* (except .env.example/.sample/.template)".into(),
        "*.pem, *.key, *.p12, *.pfx, *.jks, *.keystore, *.kdbx".into(),
        "id_rsa, id_ed25519, *credentials*.json, secrets.yml/.yaml".into(),
        ".npmrc, .pypirc, .netrc".into(),
    ];
    let caches = vec![
        "node_modules/**, target/**, dist/**, build/**, out/**".into(),
        ".next/**, .nuxt/**, .svelte-kit/**, .turbo/**, .cache/**".into(),
        "__pycache__/**, .venv/**, venv/**, .pytest_cache/**, .mypy_cache/**".into(),
        "vendor/**, .gradle/**, obj/**, .idea/**, .vs/**".into(),
        "*.log, *.tmp, *.swp, *.swo, .DS_Store, Thumbs.db".into(),
    ];
    (security, caches)
}
