use std::fs::{self, File};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};

pub const TEMP_FILE_PREFIX: &str = ".agentrestore-tmp-";

/// Canonicalizes an existing directory path and ensures it is a real directory (not a symlink).
pub fn validate_project_root(root: &Path) -> CoreResult<PathBuf> {
    let meta = fs::symlink_metadata(root)
        .map_err(|_| CoreError::InvalidProjectDirectory(root.display().to_string()))?;
    if meta.file_type().is_symlink() {
        return Err(CoreError::UnsafeSymlink(root.display().to_string()));
    }
    if !meta.is_dir() {
        return Err(CoreError::InvalidProjectDirectory(
            root.display().to_string(),
        ));
    }
    let canonical = fs::canonicalize(root)
        .map_err(|_| CoreError::InvalidProjectDirectory(root.display().to_string()))?;
    Ok(strip_windows_verbatim_prefix(canonical))
}

/// Normalizes Windows `\\?\` extended prefixes so paths remain clean and comparable across OSes.
pub fn strip_windows_verbatim_prefix(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let s = path.to_string_lossy();
        if let Some(stripped) = s.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{}", stripped));
        }
        if let Some(stripped) = s.strip_prefix(r"\\?\") {
            return PathBuf::from(stripped);
        }
    }
    path
}

/// Validates a relative path string from a checkpoint or user input.
/// Rejects absolute paths, drive letters, `..`, null bytes, and any `.git` component.
pub fn normalize_and_validate_rel_path(rel: &str) -> CoreResult<String> {
    if rel.is_empty() || rel.contains('\0') {
        return Err(CoreError::PathTraversal(rel.to_string()));
    }

    let unified = rel.replace('\\', "/");
    if unified.starts_with('/') || unified.starts_with("//") {
        return Err(CoreError::PathTraversal(rel.to_string()));
    }

    // Reject Windows drive prefixes such as C:foo
    if unified.len() >= 2 && unified.as_bytes()[1] == b':' {
        return Err(CoreError::PathTraversal(rel.to_string()));
    }

    let mut clean_parts = Vec::new();
    for part in unified.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err(CoreError::PathTraversal(rel.to_string()));
        }
        if part.eq_ignore_ascii_case(".git") {
            return Err(CoreError::ProtectedGitPath(rel.to_string()));
        }
        if part.starts_with(TEMP_FILE_PREFIX) {
            return Err(CoreError::UnauthorizedPath(rel.to_string()));
        }
        clean_parts.push(part);
    }

    if clean_parts.is_empty() {
        return Err(CoreError::PathTraversal(rel.to_string()));
    }

    Ok(clean_parts.join("/"))
}

/// Resolves a validated relative path inside a canonical project root,
/// verifying that NO existing ancestor or target component is a symbolic link
/// and that the final target remains strictly inside `project_root`.
pub fn resolve_safe_project_path(project_root: &Path, rel_path: &str) -> CoreResult<PathBuf> {
    let normalized = normalize_and_validate_rel_path(rel_path)?;
    let canonical_root = validate_project_root(project_root)?;

    let mut current = canonical_root.clone();
    for segment in normalized.split('/') {
        if segment.eq_ignore_ascii_case(".git") {
            return Err(CoreError::ProtectedGitPath(normalized));
        }
        current.push(segment);

        // If the segment already exists on disk, ensure it is NOT a symlink
        match fs::symlink_metadata(&current) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return Err(CoreError::UnsafeSymlink(current.display().to_string()));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Non-existent leaf/subdir is fine; will be created safely
            }
            Err(e) => return Err(CoreError::Io(e)),
        }
    }

    // Verify lexical containment against canonical root
    let rel_check = current
        .strip_prefix(&canonical_root)
        .map_err(|_| CoreError::UnauthorizedPath(current.display().to_string()))?;

    for comp in rel_check.components() {
        match comp {
            Component::Normal(name) => {
                if name.to_string_lossy().eq_ignore_ascii_case(".git") {
                    return Err(CoreError::ProtectedGitPath(normalized));
                }
            }
            _ => return Err(CoreError::PathTraversal(normalized)),
        }
    }

    Ok(current)
}

/// Converts a filesystem path inside `canonical_root` into a normalized forward-slash relative path.
pub fn path_to_safe_rel(canonical_root: &Path, full_path: &Path) -> CoreResult<String> {
    let clean_full = strip_windows_verbatim_prefix(full_path.to_path_buf());
    let rel = clean_full
        .strip_prefix(canonical_root)
        .map_err(|_| CoreError::UnauthorizedPath(full_path.display().to_string()))?;
    let rel_str = rel.to_string_lossy().replace('\\', "/");
    normalize_and_validate_rel_path(&rel_str)
}

/// Ensures no symlink exists on any parent directory before creating missing directories inside project root.
pub fn ensure_safe_parent_dirs(project_root: &Path, target_file: &Path) -> CoreResult<()> {
    let canonical_root = validate_project_root(project_root)?;
    let parent = target_file
        .parent()
        .ok_or_else(|| CoreError::UnauthorizedPath(target_file.display().to_string()))?;

    let rel_parent = parent
        .strip_prefix(&canonical_root)
        .map_err(|_| CoreError::UnauthorizedPath(target_file.display().to_string()))?;

    let mut cur = canonical_root;
    for comp in rel_parent.components() {
        let seg = match comp {
            Component::Normal(s) => s,
            _ => return Err(CoreError::PathTraversal(target_file.display().to_string())),
        };
        if seg.to_string_lossy().eq_ignore_ascii_case(".git") {
            return Err(CoreError::ProtectedGitPath(
                target_file.display().to_string(),
            ));
        }
        cur.push(seg);
        match fs::symlink_metadata(&cur) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return Err(CoreError::UnsafeSymlink(cur.display().to_string()));
                }
                if !meta.is_dir() {
                    return Err(CoreError::UnauthorizedPath(format!(
                        "Expected directory at {}",
                        cur.display()
                    )));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&cur)?;
            }
            Err(e) => return Err(CoreError::Io(e)),
        }
    }
    Ok(())
}

/// Atomically writes `data` to `target_path` inside `project_root` via a sibling temp file + sync_all + rename.
/// Handles Windows replacement semantics safely without silently losing the target if an error occurs.
pub fn atomic_write_project_file(
    project_root: &Path,
    rel_path: &str,
    data: &[u8],
    permissions: Option<u32>,
) -> CoreResult<()> {
    let target_path = resolve_safe_project_path(project_root, rel_path)?;
    ensure_safe_parent_dirs(project_root, &target_path)?;

    // Double-check target itself is not a symlink right before write
    if let Ok(meta) = fs::symlink_metadata(&target_path) {
        if meta.file_type().is_symlink() {
            return Err(CoreError::UnsafeSymlink(target_path.display().to_string()));
        }
    }

    let parent = target_path
        .parent()
        .ok_or_else(|| CoreError::UnauthorizedPath(rel_path.to_string()))?;
    let tmp_name = format!("{}{}", TEMP_FILE_PREFIX, Uuid::new_v4().simple());
    let tmp_path = parent.join(tmp_name);

    {
        let mut f = File::create(&tmp_path)?;
        f.write_all(data)?;
        f.sync_all()?;
    }

    #[cfg(unix)]
    if let Some(mode) = permissions {
        use std::os::unix::fs::PermissionsExt;
        if mode != 0 {
            let _ = fs::set_permissions(&tmp_path, fs::Permissions::from_mode(mode));
        }
    }
    #[cfg(not(unix))]
    let _ = permissions;

    if let Err(err) = atomic_replace_file(&tmp_path, &target_path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(err);
    }

    Ok(())
}

/// Safely replaces `target` with `tmp` across platforms.
/// On Unix `fs::rename` atomically replaces `target`.
/// On Windows, `fs::rename` fails if `target` exists or is read-only; we swap safely with rollback.
pub fn atomic_replace_file(tmp: &Path, target: &Path) -> CoreResult<()> {
    #[cfg(unix)]
    {
        fs::rename(tmp, target)?;
        Ok(())
    }

    #[cfg(windows)]
    {
        // Try direct rename first (succeeds when target does not exist)
        if fs::rename(tmp, target).is_ok() {
            return Ok(());
        }

        // Target exists: clear read-only attribute if needed, move old target to backup name, rename tmp -> target, remove backup
        if let Ok(meta) = fs::symlink_metadata(target) {
            if meta.file_type().is_symlink() {
                return Err(CoreError::UnsafeSymlink(target.display().to_string()));
            }
            let mut perms = meta.permissions();
            if perms.readonly() {
                perms.set_readonly(false);
                let _ = fs::set_permissions(target, perms);
            }
        }

        let parent = target.parent().unwrap_or_else(|| Path::new("."));
        let backup_path = parent.join(format!(
            "{}bak-{}",
            TEMP_FILE_PREFIX,
            Uuid::new_v4().simple()
        ));

        fs::rename(target, &backup_path)?;
        match fs::rename(tmp, target) {
            Ok(()) => {
                let _ = fs::remove_file(&backup_path);
                Ok(())
            }
            Err(e) => {
                // Rollback original file so user never loses data
                let _ = fs::rename(&backup_path, target);
                Err(CoreError::Io(e))
            }
        }
    }
}

/// Removes stale `.agentrestore-tmp-*` files left behind in a project directory after a crash.
pub fn cleanup_project_temp_files(project_root: &Path) -> u64 {
    let mut cleaned = 0u64;
    for entry in walkdir::WalkDir::new(project_root)
        .follow_links(false)
        .into_iter()
        .flatten()
    {
        let name = entry.file_name().to_string_lossy();
        if name.eq_ignore_ascii_case(".git") {
            continue;
        }
        if name.starts_with(TEMP_FILE_PREFIX) && entry.file_type().is_file() {
            if fs::remove_file(entry.path()).is_ok() {
                cleaned += 1;
            }
        }
    }
    cleaned
}

/// Checks if a given byte slice looks like a binary file (null byte in first 8 KiB or invalid UTF-8).
pub fn is_probably_binary(data: &[u8]) -> bool {
    let sample_len = data.len().min(8192);
    let sample = &data[..sample_len];
    if sample.contains(&0) {
        return true;
    }
    std::str::from_utf8(sample).is_err()
}
