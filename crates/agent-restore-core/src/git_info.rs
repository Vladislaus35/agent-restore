use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Default)]
pub struct GitSnapshotInfo {
    pub is_git_repo: bool,
    pub branch: Option<String>,
    pub commit_short: Option<String>,
    pub is_dirty: bool,
    pub file_statuses: HashMap<String, String>,
}

/// Strictly read-only Git metadata inspection.
/// Uses `GIT_OPTIONAL_LOCKS=0` so Git NEVER creates index locks or writes anything inside `.git`.
pub fn inspect_git_readonly(project_root: &Path) -> GitSnapshotInfo {
    if !project_root.join(".git").exists() {
        return GitSnapshotInfo::default();
    }

    let branch = run_git_readonly(project_root, &["rev-parse", "--abbrev-ref", "HEAD"])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let commit_short = run_git_readonly(project_root, &["rev-parse", "--short", "HEAD"])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let mut file_statuses = HashMap::new();
    let mut is_dirty = false;

    if let Some(status_out) = run_git_readonly(
        project_root,
        &[
            "status",
            "--porcelain",
            "--untracked-files=all",
            "-z",
        ],
    ) {
        let mut entries = status_out.split('\0').peekable();
        while let Some(entry) = entries.next() {
            if entry.len() < 4 {
                continue;
            }
            is_dirty = true;
            let code = &entry[0..2];
            let path = entry[3..].replace('\\', "/");

            // In `-z` porcelain mode, renames/copies include an extra NUL-terminated original path
            if code.starts_with('R') || code.starts_with('C') {
                let _ = entries.next();
            }

            let status_label = if code == "??" {
                "untracked"
            } else if code.contains('A') {
                "staged"
            } else if code.contains('D') {
                "deleted"
            } else {
                "modified"
            };
            file_statuses.insert(path, status_label.to_string());
        }
    }

    GitSnapshotInfo {
        is_git_repo: true,
        branch,
        commit_short,
        is_dirty,
        file_statuses,
    }
}

fn run_git_readonly(project_root: &Path, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new("git");
    cmd.current_dir(project_root)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .arg("--no-optional-locks")
        .args(args);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}
