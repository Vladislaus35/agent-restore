use chrono::{DateTime, Duration, Utc};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;
use uuid::Uuid;
use walkdir::WalkDir;

use crate::cas::CasStore;
use crate::db::Database;
use crate::diff::{compute_side_by_side_diff, diff_file_maps};
use crate::error::{CoreError, CoreResult};
use crate::git_info::inspect_git_readonly;
use crate::ignore::IgnoreMatcher;
use crate::models::{
    Checkpoint, CheckpointDiffSummary, CheckpointFile, CrashRecoveryReport,
    CreateCheckpointOptions, CreateCheckpointOutcome, CreateProjectRequest, ExecuteRestoreRequest,
    FileDiffResult, GcReport, GlobalSettings, IntegrityIssue, IntegrityReport, Project,
    RestoreFilePlan, RestorePreview, RestoreResult, StorageProjectBreakdown, StorageStats,
    TriggerType, UpdateProjectRequest, WorkspaceStatus,
};
use crate::security::{
    atomic_write_project_file, cleanup_project_temp_files, is_probably_binary,
    normalize_and_validate_rel_path, path_to_safe_rel, resolve_safe_project_path,
    validate_project_root,
};

pub struct AgentRestoreEngine {
    data_dir: PathBuf,
    cas: CasStore,
    db: Mutex<Database>,
    startup_recovery: Mutex<CrashRecoveryReport>,
    /// Serializes backup/restore operations per project to avoid concurrent corruption.
    project_locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

impl AgentRestoreEngine {
    pub fn open(data_dir: impl AsRef<Path>) -> CoreResult<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();
        fs::create_dir_all(&data_dir)?;
        let cas = CasStore::open(&data_dir)?;
        let mut db = Database::open(&data_dir.join("agent_restore.db"))?;

        // Run automatic crash recovery on startup
        let (cleaned_incomplete_checkpoints, interrupted_restores) =
            db.recover_incomplete_state()?;
        let mut cleaned_temp_files = cas.cleanup_tmp_dir();

        // Also clean any leftover staging temp files inside registered projects
        if let Ok(projects) = db.list_projects() {
            for p in projects {
                let root = Path::new(&p.root_path);
                if root.is_dir() {
                    cleaned_temp_files += cleanup_project_temp_files(root);
                }
            }
        }

        let recovery_report = CrashRecoveryReport {
            cleaned_incomplete_checkpoints,
            cleaned_temp_files,
            interrupted_restores,
        };

        Ok(Self {
            data_dir,
            cas,
            db: Mutex::new(db),
            startup_recovery: Mutex::new(recovery_report),
            project_locks: Mutex::new(HashMap::new()),
        })
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn cas(&self) -> &CasStore {
        &self.cas
    }

    pub fn get_crash_recovery_report(&self) -> CoreResult<CrashRecoveryReport> {
        let guard = self
            .startup_recovery
            .lock()
            .map_err(|_| CoreError::LockPoisoned)?;
        Ok(guard.clone())
    }

    pub fn dismiss_crash_recovery_notices(&self) -> CoreResult<()> {
        let mut guard = self
            .startup_recovery
            .lock()
            .map_err(|_| CoreError::LockPoisoned)?;
        guard.interrupted_restores.clear();
        Ok(())
    }

    fn project_lock(&self, project_id: &str) -> CoreResult<Arc<Mutex<()>>> {
        let mut map = self
            .project_locks
            .lock()
            .map_err(|_| CoreError::LockPoisoned)?;
        Ok(map
            .entry(project_id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone())
    }

    pub fn get_settings(&self) -> CoreResult<GlobalSettings> {
        let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
        db.get_settings()
    }

    pub fn save_settings(&self, settings: GlobalSettings) -> CoreResult<GlobalSettings> {
        IgnoreMatcher::validate_patterns(&settings.global_excludes)?;
        let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
        db.save_settings(&settings)?;
        Ok(settings)
    }

    pub fn add_project(&self, req: CreateProjectRequest) -> CoreResult<Project> {
        let canonical_root = validate_project_root(Path::new(&req.root_path))?;
        let root_str = canonical_root.to_string_lossy().to_string();

        if let Some(ref patterns) = req.custom_excludes {
            IgnoreMatcher::validate_patterns(patterns)?;
        }

        let settings = self.get_settings()?;
        let default_name = canonical_root
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Project".to_string());

        let name = req
            .name
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or(default_name);

        let now = Utc::now().to_rfc3339();
        let project = Project {
            id: Uuid::new_v4().to_string(),
            name,
            root_path: root_str.clone(),
            enabled: true,
            auto_backup: req.auto_backup.unwrap_or(true),
            interval_secs: req
                .interval_secs
                .unwrap_or(settings.default_interval_secs)
                .max(10),
            debounce_ms: req
                .debounce_ms
                .unwrap_or(settings.default_debounce_ms)
                .max(300),
            max_storage_mb: req
                .max_storage_mb
                .unwrap_or(settings.default_max_storage_mb)
                .max(16),
            max_checkpoints: req
                .max_checkpoints
                .unwrap_or(settings.default_max_checkpoints)
                .max(2),
            retention_days: req
                .retention_days
                .unwrap_or(settings.default_retention_days)
                .max(1),
            custom_excludes: req.custom_excludes.unwrap_or_default(),
            respect_gitignore: req.respect_gitignore.unwrap_or(true),
            git_integration: req.git_integration.unwrap_or(true),
            max_file_size_mb: req
                .max_file_size_mb
                .unwrap_or(settings.default_max_file_size_mb)
                .max(1),
            created_at: now.clone(),
            updated_at: now,
            last_checkpoint_at: None,
            checkpoint_count: 0,
            stored_size_bytes: 0,
            logical_size_bytes: 0,
            git_branch: None,
            git_commit: None,
            git_dirty: false,
        };

        {
            let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
            if db.find_project_by_root(&root_str)?.is_some() {
                return Err(CoreError::ProjectAlreadyExists(root_str));
            }
            db.insert_project(&project)?;
        }

        // Create initial baseline checkpoint automatically so the developer is protected immediately
        let _ = self.create_checkpoint(CreateCheckpointOptions {
            project_id: project.id.clone(),
            label: Some("Initial Baseline".to_string()),
            note: Some("Automatic initial checkpoint created when registering project.".to_string()),
            trigger_type: TriggerType::Auto,
            force_if_unchanged: true,
        });

        self.get_project(&project.id)
    }

    pub fn list_projects(&self) -> CoreResult<Vec<Project>> {
        let mut projects = {
            let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
            db.list_projects()?
        };
        for p in &mut projects {
            if p.git_integration {
                let git = inspect_git_readonly(Path::new(&p.root_path));
                p.git_branch = git.branch;
                p.git_commit = git.commit_short;
                p.git_dirty = git.is_dirty;
            }
        }
        Ok(projects)
    }

    pub fn get_project(&self, project_id: &str) -> CoreResult<Project> {
        let mut p = {
            let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
            db.get_project(project_id)?
        };
        if p.git_integration {
            let git = inspect_git_readonly(Path::new(&p.root_path));
            p.git_branch = git.branch;
            p.git_commit = git.commit_short;
            p.git_dirty = git.is_dirty;
        }
        Ok(p)
    }

    pub fn update_project(
        &self,
        project_id: &str,
        req: UpdateProjectRequest,
    ) -> CoreResult<Project> {
        if let Some(ref patterns) = req.custom_excludes {
            IgnoreMatcher::validate_patterns(patterns)?;
        }

        let mut project = {
            let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
            db.get_project(project_id)?
        };

        if let Some(name) = req.name {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                project.name = trimmed.to_string();
            }
        }
        if let Some(enabled) = req.enabled {
            project.enabled = enabled;
        }
        if let Some(auto_backup) = req.auto_backup {
            project.auto_backup = auto_backup;
        }
        if let Some(interval) = req.interval_secs {
            project.interval_secs = interval.max(10);
        }
        if let Some(debounce) = req.debounce_ms {
            project.debounce_ms = debounce.max(300);
        }
        if let Some(max_mb) = req.max_storage_mb {
            project.max_storage_mb = max_mb.max(16);
        }
        if let Some(max_cp) = req.max_checkpoints {
            project.max_checkpoints = max_cp.max(2);
        }
        if let Some(ret) = req.retention_days {
            project.retention_days = ret.max(1);
        }
        if let Some(excludes) = req.custom_excludes {
            project.custom_excludes = excludes;
        }
        if let Some(rg) = req.respect_gitignore {
            project.respect_gitignore = rg;
        }
        if let Some(gi) = req.git_integration {
            project.git_integration = gi;
        }
        if let Some(mfs) = req.max_file_size_mb {
            project.max_file_size_mb = mfs.max(1);
        }
        project.updated_at = Utc::now().to_rfc3339();

        {
            let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
            db.update_project(&project)?;
        }

        self.get_project(project_id)
    }

    pub fn remove_project(&self, project_id: &str) -> CoreResult<GcReport> {
        let lock = self.project_lock(project_id)?;
        let _guard = lock.lock().map_err(|_| CoreError::LockPoisoned)?;
        {
            let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
            db.delete_project(project_id)?;
        }
        self.run_gc()
    }

    /// Creates an incremental, content-addressed, compressed checkpoint for a registered project.
    pub fn create_checkpoint(
        &self,
        opts: CreateCheckpointOptions,
    ) -> CoreResult<CreateCheckpointOutcome> {
        let lock = self.project_lock(&opts.project_id)?;
        let _guard = lock.lock().map_err(|_| CoreError::LockPoisoned)?;
        self.create_checkpoint_locked(opts)
    }

    fn create_checkpoint_locked(
        &self,
        opts: CreateCheckpointOptions,
    ) -> CoreResult<CreateCheckpointOutcome> {
        let (project, settings, prev_checkpoint, prev_files) = {
            let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
            let project = db.get_project(&opts.project_id)?;
            let settings = db.get_settings()?;
            let prev = db.get_latest_checkpoint(&opts.project_id)?;
            let prev_files = match &prev {
                Some(cp) => db.get_checkpoint_file_map(&cp.id)?,
                None => HashMap::new(),
            };
            (project, settings, prev, prev_files)
        };

        let root_path = validate_project_root(Path::new(&project.root_path))?;
        let min_free_bytes = settings.min_free_disk_mb.saturating_mul(1024 * 1024);

        // Pre-flight disk space check before writing anything
        CasStore::check_disk_space(self.cas.objects_dir(), 1024 * 1024, min_free_bytes)?;

        let matcher = IgnoreMatcher::build(
            &root_path,
            &settings.global_excludes,
            &project.custom_excludes,
            project.respect_gitignore,
            project.max_file_size_mb,
        )?;

        let git_info = if project.git_integration {
            inspect_git_readonly(&root_path)
        } else {
            Default::default()
        };

        let checkpoint_id = Uuid::new_v4().to_string();
        let created_at = Utc::now().to_rfc3339();
        let label = opts
            .label
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| default_checkpoint_label(opts.trigger_type));
        let note = opts.note.unwrap_or_default();

        // Mark checkpoint in_progress for crash visibility
        {
            let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
            db.begin_checkpoint_record(
                &checkpoint_id,
                &project.id,
                &label,
                &note,
                opts.trigger_type,
                prev_checkpoint.as_ref().map(|c| c.id.as_str()),
                &created_at,
            )?;
        }

        let scan_result = self.scan_and_store_files(
            &root_path,
            &matcher,
            &prev_files,
            &git_info.file_statuses,
            git_info.is_git_repo,
            min_free_bytes,
        );

        let (files, new_objects, stored_size_bytes, warnings) = match scan_result {
            Ok(v) => v,
            Err(e) => {
                if let Ok(db) = self.db.lock() {
                    let _ = db.abort_in_progress_checkpoint(&checkpoint_id);
                }
                return Err(e);
            }
        };

        // Compute added/modified/deleted relative to parent checkpoint
        let current_map: HashMap<String, &CheckpointFile> =
            files.iter().map(|f| (f.rel_path.clone(), f)).collect();
        let mut added_count = 0u64;
        let mut modified_count = 0u64;
        let mut deleted_count = 0u64;

        for (path, cur) in &current_map {
            match prev_files.get(path) {
                None => added_count += 1,
                Some(prev) if prev.object_hash != cur.object_hash => modified_count += 1,
                _ => {}
            }
        }
        for prev_path in prev_files.keys() {
            if !current_map.contains_key(prev_path) {
                deleted_count += 1;
            }
        }

        let total_changes = added_count + modified_count + deleted_count;
        if prev_checkpoint.is_some() && total_changes == 0 && !opts.force_if_unchanged {
            let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
            db.abort_in_progress_checkpoint(&checkpoint_id)?;
            return Ok(CreateCheckpointOutcome {
                checkpoint: None,
                skipped_unchanged: true,
                warnings,
            });
        }

        let total_size_bytes: u64 = files.iter().map(|f| f.size_bytes).sum();
        let tree_hash = compute_tree_hash(&files);

        let checkpoint = Checkpoint {
            id: checkpoint_id.clone(),
            project_id: project.id.clone(),
            label,
            note,
            trigger_type: opts.trigger_type,
            status: "complete".to_string(),
            parent_checkpoint_id: prev_checkpoint.map(|c| c.id),
            file_count: files.len() as u64,
            total_size_bytes,
            stored_size_bytes,
            added_count,
            modified_count,
            deleted_count,
            git_branch: git_info.branch,
            git_commit: git_info.commit_short,
            git_dirty: git_info.is_dirty,
            tree_hash,
            created_at,
        };

        {
            let mut db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
            db.finalize_checkpoint_transaction(&checkpoint, &files, &new_objects)?;
        }

        // Enforce retention policy for auto/watcher checkpoints (keep pre_restore & manual safe until needed)
        if matches!(opts.trigger_type, TriggerType::Auto | TriggerType::Watcher) {
            let _ = self.apply_retention_for_project_unlocked(&project.id);
        }

        Ok(CreateCheckpointOutcome {
            checkpoint: Some(checkpoint),
            skipped_unchanged: false,
            warnings,
        })
    }

    fn scan_and_store_files(
        &self,
        root_path: &Path,
        matcher: &IgnoreMatcher,
        prev_files: &HashMap<String, CheckpointFile>,
        git_statuses: &HashMap<String, String>,
        is_git_repo: bool,
        min_free_bytes: u64,
    ) -> CoreResult<(
        Vec<CheckpointFile>,
        Vec<(String, u64, u64, String)>,
        u64,
        Vec<String>,
    )> {
        let mut files = Vec::new();
        let mut new_objects = Vec::new();
        let mut seen_new_hashes = HashSet::new();
        let mut warnings = Vec::new();
        let mut new_stored_bytes = 0u64;

        let mut walker = WalkDir::new(root_path).follow_links(false).into_iter();
        while let Some(entry_res) = walker.next() {
            let entry = match entry_res {
                Ok(e) => e,
                Err(err) => {
                    warnings.push(format!("Skipped unreadable entry: {}", err));
                    continue;
                }
            };

            if entry.depth() == 0 {
                continue;
            }

            // Never follow or archive symbolic links
            if entry.file_type().is_symlink() {
                if entry.file_type().is_dir() {
                    walker.skip_current_dir();
                }
                continue;
            }

            let rel_path = match path_to_safe_rel(root_path, entry.path()) {
                Ok(r) => r,
                Err(_) => {
                    if entry.file_type().is_dir() {
                        walker.skip_current_dir();
                    }
                    continue;
                }
            };

            let is_dir = entry.file_type().is_dir();
            if matcher.is_excluded(&rel_path, is_dir) {
                if is_dir {
                    walker.skip_current_dir();
                }
                continue;
            }

            if is_dir || !entry.file_type().is_file() {
                continue;
            }

            let meta = match fs::symlink_metadata(entry.path()) {
                Ok(m) => m,
                Err(e) => {
                    warnings.push(format!("Could not stat {}: {}", rel_path, e));
                    continue;
                }
            };

            if meta.file_type().is_symlink() || !meta.is_file() {
                continue;
            }

            let size = meta.len();
            if matcher.exceeds_max_file_size(size) {
                warnings.push(format!(
                    "Skipped {} ({} bytes exceeds max file size limit)",
                    rel_path, size
                ));
                continue;
            }

            let (mtime_secs, mtime_nanos) = extract_mtime(&meta);
            let perms = extract_permissions(&meta);
            let git_status = if is_git_repo {
                git_statuses
                    .get(&rel_path)
                    .cloned()
                    .unwrap_or_else(|| "tracked".to_string())
            } else {
                "none".to_string()
            };

            // Fast incremental reuse when size & mtime match previous checkpoint and CAS object exists
            if let Some(prev) = prev_files.get(&rel_path) {
                if prev.size_bytes == size
                    && prev.mtime_secs == mtime_secs
                    && prev.mtime_nanos == mtime_nanos
                    && self.cas.has_object(&prev.object_hash)
                {
                    files.push(CheckpointFile {
                        rel_path,
                        object_hash: prev.object_hash.clone(),
                        size_bytes: size,
                        mtime_secs,
                        mtime_nanos,
                        permissions: perms,
                        is_binary: prev.is_binary,
                        git_status,
                    });
                    continue;
                }
            }

            // Read file with concurrent-modification detection (retry up to 2 times if mutated mid-read)
            let (data, final_size, final_mtime_secs, final_mtime_nanos) =
                match read_file_stably(entry.path()) {
                    Ok(v) => v,
                    Err(e) => {
                        warnings.push(format!("Skipped {} due to read error: {}", rel_path, e));
                        continue;
                    }
                };

            let is_binary = is_probably_binary(&data);
            let stored = self.cas.put_bytes(&data, &rel_path, min_free_bytes)?;

            if stored.newly_written && seen_new_hashes.insert(stored.hash.clone()) {
                new_stored_bytes += stored.stored_size;
                new_objects.push((
                    stored.hash.clone(),
                    stored.raw_size,
                    stored.stored_size,
                    stored.compression.to_string(),
                ));
            } else if !stored.newly_written && seen_new_hashes.insert(stored.hash.clone()) {
                // Ensure metadata row exists in SQLite even if object existed on disk from a prior crashed write
                new_objects.push((
                    stored.hash.clone(),
                    stored.raw_size,
                    stored.stored_size,
                    stored.compression.to_string(),
                ));
            }

            files.push(CheckpointFile {
                rel_path,
                object_hash: stored.hash,
                size_bytes: final_size,
                mtime_secs: final_mtime_secs,
                mtime_nanos: final_mtime_nanos,
                permissions: perms,
                is_binary,
                git_status,
            });
        }

        files.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
        Ok((files, new_objects, new_stored_bytes, warnings))
    }

    pub fn list_checkpoints(
        &self,
        project_id: &str,
        query: Option<&str>,
        trigger_filter: Option<&str>,
        limit: Option<u32>,
    ) -> CoreResult<Vec<Checkpoint>> {
        let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
        db.list_checkpoints(project_id, query, trigger_filter, limit)
    }

    pub fn get_checkpoint(&self, checkpoint_id: &str) -> CoreResult<Checkpoint> {
        let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
        db.get_checkpoint(checkpoint_id)
    }

    pub fn rename_checkpoint(
        &self,
        checkpoint_id: &str,
        label: &str,
        note: Option<&str>,
    ) -> CoreResult<Checkpoint> {
        let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
        db.rename_checkpoint(checkpoint_id, label, note)
    }

    pub fn delete_checkpoint(&self, checkpoint_id: &str) -> CoreResult<GcReport> {
        {
            let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
            db.delete_checkpoint(checkpoint_id)?;
        }
        self.run_gc()
    }

    pub fn get_checkpoint_files(&self, checkpoint_id: &str) -> CoreResult<Vec<CheckpointFile>> {
        let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
        db.get_checkpoint_files(checkpoint_id)
    }

    /// Computes file changes between a checkpoint and its parent (or another checkpoint).
    pub fn diff_checkpoints(
        &self,
        base_checkpoint_id: Option<&str>,
        target_checkpoint_id: &str,
    ) -> CoreResult<CheckpointDiffSummary> {
        let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
        let target_cp = db.get_checkpoint(target_checkpoint_id)?;
        let resolved_base = base_checkpoint_id
            .map(|s| s.to_string())
            .or(target_cp.parent_checkpoint_id);

        let base_map = match &resolved_base {
            Some(bid) => db.get_checkpoint_file_map(bid).unwrap_or_default(),
            None => HashMap::new(),
        };
        let target_map = db.get_checkpoint_file_map(target_checkpoint_id)?;

        Ok(diff_file_maps(
            &base_map,
            &target_map,
            resolved_base,
            Some(target_checkpoint_id.to_string()),
            false,
        ))
    }

    /// Compares a checkpoint against the live workspace files on disk.
    pub fn diff_checkpoint_with_workspace(
        &self,
        project_id: &str,
        checkpoint_id: &str,
    ) -> CoreResult<CheckpointDiffSummary> {
        let (project, settings, cp_map) = {
            let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
            let project = db.get_project(project_id)?;
            let settings = db.get_settings()?;
            let cp = db.get_checkpoint(checkpoint_id)?;
            if cp.project_id != project_id {
                return Err(CoreError::UnauthorizedPath(
                    "Checkpoint does not belong to this project".to_string(),
                ));
            }
            let cp_map = db.get_checkpoint_file_map(checkpoint_id)?;
            (project, settings, cp_map)
        };

        let workspace_map = self.scan_workspace_readonly(&project, &settings, &cp_map)?;
        Ok(diff_file_maps(
            &cp_map,
            &workspace_map,
            Some(checkpoint_id.to_string()),
            None,
            true,
        ))
    }

    /// Returns a quick live status of uncommitted workspace changes relative to the latest checkpoint.
    pub fn get_workspace_status(&self, project_id: &str) -> CoreResult<WorkspaceStatus> {
        let (project, settings, latest_cp, latest_map) = {
            let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
            let project = db.get_project(project_id)?;
            let settings = db.get_settings()?;
            let latest = db.get_latest_checkpoint(project_id)?;
            let map = match &latest {
                Some(cp) => db.get_checkpoint_file_map(&cp.id)?,
                None => HashMap::new(),
            };
            (project, settings, latest, map)
        };

        let workspace_map = self.scan_workspace_readonly(&project, &settings, &latest_map)?;
        let untracked_file_count = workspace_map
            .values()
            .filter(|f| f.git_status == "untracked")
            .count() as u64;

        let summary = diff_file_maps(
            &latest_map,
            &workspace_map,
            latest_cp.map(|c| c.id),
            None,
            true,
        );

        Ok(WorkspaceStatus {
            project_id: project_id.to_string(),
            has_pending_changes: summary.added_count + summary.modified_count + summary.deleted_count
                > 0,
            added_count: summary.added_count,
            modified_count: summary.modified_count,
            deleted_count: summary.deleted_count,
            tracked_file_count: workspace_map.len() as u64,
            untracked_file_count,
        })
    }

    /// Produces a side-by-side line diff for a specific file:
    /// - If `compare_with_workspace` is true: checkpoint vs live workspace file
    /// - Otherwise: parent checkpoint (or explicit `base_checkpoint_id`) vs `checkpoint_id`
    pub fn get_file_diff(
        &self,
        project_id: &str,
        checkpoint_id: &str,
        rel_path: &str,
        base_checkpoint_id: Option<&str>,
        compare_with_workspace: bool,
    ) -> CoreResult<FileDiffResult> {
        let normalized = normalize_and_validate_rel_path(rel_path)?;
        let (project, cp, cp_files, base_cp_opt, base_files) = {
            let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
            let project = db.get_project(project_id)?;
            let cp = db.get_checkpoint(checkpoint_id)?;
            if cp.project_id != project_id {
                return Err(CoreError::UnauthorizedPath(
                    "Checkpoint does not belong to this project".to_string(),
                ));
            }
            let cp_files = db.get_checkpoint_file_map(checkpoint_id)?;

            let resolved_base_id = if compare_with_workspace {
                None
            } else {
                base_checkpoint_id
                    .map(|s| s.to_string())
                    .or_else(|| cp.parent_checkpoint_id.clone())
            };
            let base_cp = match &resolved_base_id {
                Some(bid) => db.get_checkpoint(bid).ok(),
                None => None,
            };
            let base_files = match &resolved_base_id {
                Some(bid) => db.get_checkpoint_file_map(bid).unwrap_or_default(),
                None => HashMap::new(),
            };
            (project, cp, cp_files, base_cp, base_files)
        };

        if compare_with_workspace {
            let left_bytes = match cp_files.get(&normalized) {
                Some(f) => Some(self.cas.get_verified_bytes(&f.object_hash)?),
                None => None,
            };
            let target_path = resolve_safe_project_path(Path::new(&project.root_path), &normalized)?;
            let right_bytes = if target_path.is_file() {
                Some(fs::read(&target_path)?)
            } else {
                None
            };
            Ok(compute_side_by_side_diff(
                &normalized,
                left_bytes.as_deref(),
                right_bytes.as_deref(),
                format!("Checkpoint: {}", cp.label),
                "Current Workspace".to_string(),
            ))
        } else {
            let left_bytes = match base_files.get(&normalized) {
                Some(f) => Some(self.cas.get_verified_bytes(&f.object_hash)?),
                None => None,
            };
            let right_bytes = match cp_files.get(&normalized) {
                Some(f) => Some(self.cas.get_verified_bytes(&f.object_hash)?),
                None => None,
            };
            let left_label = base_cp_opt
                .map(|b| format!("Previous: {}", b.label))
                .unwrap_or_else(|| "Empty (Initial)".to_string());
            Ok(compute_side_by_side_diff(
                &normalized,
                left_bytes.as_deref(),
                right_bytes.as_deref(),
                left_label,
                format!("Checkpoint: {}", cp.label),
            ))
        }
    }

    /// Generates an exact, non-destructive restore preview with cryptographic integrity verification
    /// and a workspace fingerprint for concurrent-modification detection.
    pub fn preview_restore(
        &self,
        project_id: &str,
        checkpoint_id: &str,
        selected_paths: Option<Vec<String>>,
        delete_extra_files: bool,
    ) -> CoreResult<RestorePreview> {
        let (project, settings, cp, cp_files) = {
            let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
            let project = db.get_project(project_id)?;
            let settings = db.get_settings()?;
            let cp = db.get_checkpoint(checkpoint_id)?;
            if cp.project_id != project_id {
                return Err(CoreError::UnauthorizedPath(
                    "Checkpoint does not belong to this project".to_string(),
                ));
            }
            let files = db.get_checkpoint_file_map(checkpoint_id)?;
            (project, settings, cp, files)
        };

        let root_path = validate_project_root(Path::new(&project.root_path))?;
        let is_full_restore = selected_paths.is_none();

        let target_files: BTreeMap<String, CheckpointFile> = match selected_paths {
            Some(paths) => {
                let mut map = BTreeMap::new();
                for p in paths {
                    let norm = normalize_and_validate_rel_path(&p)?;
                    // Also check path containment & symlink safety right now
                    let _ = resolve_safe_project_path(&root_path, &norm)?;
                    let entry = cp_files
                        .get(&norm)
                        .ok_or_else(|| CoreError::FileNotInCheckpoint(norm.clone()))?;
                    map.insert(norm, entry.clone());
                }
                map
            }
            None => cp_files.clone().into_iter().collect(),
        };

        let workspace_map = self.scan_workspace_readonly(&project, &settings, &cp_files)?;

        let mut files_to_recreate = Vec::new();
        let mut files_to_overwrite = Vec::new();
        let mut unchanged_count = 0u64;
        let mut total_bytes_to_write = 0u64;
        let mut affected_paths: Vec<String> = Vec::new();
        let mut corrupted_objects: Vec<String> = Vec::new();

        for (rel_path, target) in &target_files {
            // Verify CAS object integrity up front
            if let Err(err) = self.cas.verify_object(&target.object_hash) {
                corrupted_objects.push(format!("{} ({}): {}", rel_path, &target.object_hash[..12], err));
            }

            match workspace_map.get(rel_path) {
                None => {
                    affected_paths.push(rel_path.clone());
                    total_bytes_to_write += target.size_bytes;
                    files_to_recreate.push(RestoreFilePlan {
                        rel_path: rel_path.clone(),
                        action: "restore_deleted".to_string(),
                        current_size: None,
                        target_size: Some(target.size_bytes),
                    });
                }
                Some(cur) if cur.object_hash != target.object_hash => {
                    affected_paths.push(rel_path.clone());
                    total_bytes_to_write += target.size_bytes;
                    files_to_overwrite.push(RestoreFilePlan {
                        rel_path: rel_path.clone(),
                        action: "overwrite".to_string(),
                        current_size: Some(cur.size_bytes),
                        target_size: Some(target.size_bytes),
                    });
                }
                Some(_) => {
                    unchanged_count += 1;
                }
            }
        }

        let mut extra_files_to_delete = Vec::new();
        let mut extra_files_kept = Vec::new();
        if is_full_restore {
            let mut extra_keys: Vec<&String> = workspace_map
                .keys()
                .filter(|k| !target_files.contains_key(*k))
                .collect();
            extra_keys.sort();
            for rel_path in extra_keys {
                let cur = &workspace_map[rel_path];
                let plan = RestoreFilePlan {
                    rel_path: rel_path.clone(),
                    action: if delete_extra_files {
                        "delete_extra".to_string()
                    } else {
                        "keep_extra".to_string()
                    },
                    current_size: Some(cur.size_bytes),
                    target_size: None,
                };
                if delete_extra_files {
                    affected_paths.push(rel_path.clone());
                    extra_files_to_delete.push(plan);
                } else {
                    extra_files_kept.push(plan);
                }
            }
        }

        affected_paths.sort();
        affected_paths.dedup();
        let workspace_fingerprint = compute_workspace_fingerprint(&root_path, &affected_paths)?;

        Ok(RestorePreview {
            project_id: project.id,
            checkpoint_id: cp.id,
            checkpoint_label: cp.label,
            is_full_restore,
            delete_extra_files: is_full_restore && delete_extra_files,
            files_to_recreate,
            files_to_overwrite,
            extra_files_to_delete,
            extra_files_kept,
            unchanged_count,
            total_bytes_to_write,
            workspace_fingerprint,
            integrity_ok: corrupted_objects.is_empty(),
            corrupted_objects,
        })
    }

    /// Executes a verified, crash-safe restore with automatic pre-restore checkpoint creation,
    /// concurrent modification detection, cryptographic hash verification, and atomic file writes.
    pub fn execute_restore(&self, req: ExecuteRestoreRequest) -> CoreResult<RestoreResult> {
        if !req.confirmed {
            return Err(CoreError::ConfirmationRequired);
        }

        let lock = self.project_lock(&req.project_id)?;
        let _guard = lock.lock().map_err(|_| CoreError::LockPoisoned)?;

        let preview = self.preview_restore(
            &req.project_id,
            &req.checkpoint_id,
            req.selected_paths.clone(),
            req.delete_extra_files,
        )?;

        if !preview.integrity_ok {
            return Err(CoreError::CorruptedSnapshot {
                hash: req.checkpoint_id.clone(),
                reason: preview.corrupted_objects.join("; "),
            });
        }

        // Concurrent modification check against preview fingerprint
        if let Some(ref expected_fp) = req.expected_fingerprint {
            if expected_fp != &preview.workspace_fingerprint {
                let mut affected = Vec::new();
                for f in preview
                    .files_to_recreate
                    .iter()
                    .chain(preview.files_to_overwrite.iter())
                    .chain(preview.extra_files_to_delete.iter())
                {
                    affected.push(f.rel_path.clone());
                }
                return Err(CoreError::ConcurrentModification { paths: affected });
            }
        }

        let (project, settings, cp_files) = {
            let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
            let project = db.get_project(&req.project_id)?;
            let settings = db.get_settings()?;
            let files = db.get_checkpoint_file_map(&req.checkpoint_id)?;
            (project, settings, files)
        };

        let root_path = validate_project_root(Path::new(&project.root_path))?;
        let min_free_bytes = settings.min_free_disk_mb.saturating_mul(1024 * 1024);

        // Verify disk space on both backup store AND target project volume BEFORE modifying anything
        CasStore::check_disk_space(
            self.cas.objects_dir(),
            preview.total_bytes_to_write.max(1024 * 1024),
            min_free_bytes,
        )?;
        CasStore::check_disk_space(&root_path, preview.total_bytes_to_write, min_free_bytes)?;

        // Pre-load & cryptographically verify all objects to restore before writing any file
        let mut to_write: Vec<(String, Vec<u8>, u32)> = Vec::new();
        for plan in preview
            .files_to_recreate
            .iter()
            .chain(preview.files_to_overwrite.iter())
        {
            let norm = normalize_and_validate_rel_path(&plan.rel_path)?;
            let _ = resolve_safe_project_path(&root_path, &norm)?;
            let meta = cp_files
                .get(&norm)
                .ok_or_else(|| CoreError::FileNotInCheckpoint(norm.clone()))?;
            let bytes = self.cas.get_verified_bytes(&meta.object_hash)?;
            to_write.push((norm, bytes, meta.permissions));
        }

        // Validate every extra file slated for deletion BEFORE touching anything
        let mut to_delete: Vec<PathBuf> = Vec::new();
        if preview.is_full_restore && preview.delete_extra_files {
            for plan in &preview.extra_files_to_delete {
                let resolved = resolve_safe_project_path(&root_path, &plan.rel_path)?;
                to_delete.push(resolved);
            }
        }

        // Step 1: Automatically create a mandatory pre-restore safety checkpoint
        let pre_restore_outcome = self.create_checkpoint_locked(CreateCheckpointOptions {
            project_id: project.id.clone(),
            label: Some(format!("Pre-Restore Safety ({})", preview.checkpoint_label)),
            note: Some(format!(
                "Automatic safety checkpoint created before restoring checkpoint {}",
                preview.checkpoint_id
            )),
            trigger_type: TriggerType::PreRestore,
            force_if_unchanged: true,
        })?;

        let pre_restore_cp = pre_restore_outcome.checkpoint.ok_or_else(|| {
            CoreError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to create mandatory pre-restore safety checkpoint",
            ))
        })?;

        // Step 2: Log restore transaction in operations_log for crash recovery
        let op_id = {
            let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
            db.start_operation(
                &project.id,
                "restore",
                &serde_json::json!({
                    "targetCheckpointId": preview.checkpoint_id,
                    "preRestoreCheckpointId": pre_restore_cp.id,
                    "filesToWrite": to_write.len(),
                    "filesToDelete": to_delete.len(),
                }),
            )?
        };

        let mut restored_count = 0u64;
        let mut deleted_count = 0u64;
        let mut warnings = pre_restore_outcome.warnings;

        // Step 3: Atomically write each restored file inside project root
        for (rel_path, bytes, perms) in to_write {
            if let Err(err) =
                atomic_write_project_file(&root_path, &rel_path, &bytes, Some(perms))
            {
                if let Ok(db) = self.db.lock() {
                    let _ = db.finish_operation(
                        &op_id,
                        "failed",
                        Some(&serde_json::json!({
                            "preRestoreCheckpointId": pre_restore_cp.id,
                            "error": err.to_string(),
                        })),
                    );
                }
                return Err(err);
            }
            restored_count += 1;
        }

        // Step 4: Delete extra files only when explicitly requested & confirmed
        for path in to_delete {
            match fs::symlink_metadata(&path) {
                Ok(meta) if meta.file_type().is_symlink() => {
                    warnings.push(format!("Skipped symlink during cleanup: {}", path.display()));
                }
                Ok(meta) if meta.is_file() => {
                    if let Err(e) = fs::remove_file(&path) {
                        warnings.push(format!("Could not remove {}: {}", path.display(), e));
                    } else {
                        deleted_count += 1;
                    }
                }
                _ => {}
            }
        }

        {
            let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
            db.finish_operation(&op_id, "completed", None)?;
        }

        Ok(RestoreResult {
            pre_restore_checkpoint: pre_restore_cp,
            restored_count,
            deleted_count,
            warnings,
        })
    }

    /// Cryptographically verifies all snapshots (or a single project's snapshots) against BLAKE3 hashes.
    pub fn verify_integrity(&self, project_id: Option<&str>) -> CoreResult<IntegrityReport> {
        let checkpoints: Vec<Checkpoint> = {
            let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
            match project_id {
                Some(pid) => db.list_checkpoints(pid, None, None, Some(10_000))?,
                None => {
                    let projects = db.list_projects()?;
                    let mut all = Vec::new();
                    for p in projects {
                        all.extend(db.list_checkpoints(&p.id, None, None, Some(10_000))?);
                    }
                    all
                }
            }
        };

        let mut issues = Vec::new();
        let mut verified_hashes: HashMap<String, Result<(), String>> = HashMap::new();

        for cp in &checkpoints {
            let files = {
                let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
                db.get_checkpoint_files(&cp.id)?
            };

            let computed_tree = compute_tree_hash(&files);
            if !cp.tree_hash.is_empty() && computed_tree != cp.tree_hash {
                issues.push(IntegrityIssue {
                    checkpoint_id: cp.id.clone(),
                    rel_path: "<tree>".to_string(),
                    object_hash: cp.tree_hash.clone(),
                    reason: format!("Checkpoint tree hash mismatch (computed {})", computed_tree),
                });
            }

            for f in files {
                let check = verified_hashes
                    .entry(f.object_hash.clone())
                    .or_insert_with(|| {
                        self.cas
                            .verify_object(&f.object_hash)
                            .map(|_| ())
                            .map_err(|e| e.to_string())
                    });
                if let Err(reason) = check {
                    issues.push(IntegrityIssue {
                        checkpoint_id: cp.id.clone(),
                        rel_path: f.rel_path,
                        object_hash: f.object_hash,
                        reason: reason.clone(),
                    });
                }
            }
        }

        Ok(IntegrityReport {
            checked_checkpoints: checkpoints.len() as u64,
            checked_objects: verified_hashes.len() as u64,
            healthy: issues.is_empty(),
            issues,
        })
    }

    pub fn apply_retention_for_project(&self, project_id: &str) -> CoreResult<GcReport> {
        let lock = self.project_lock(project_id)?;
        let _guard = lock.lock().map_err(|_| CoreError::LockPoisoned)?;
        self.apply_retention_for_project_unlocked(project_id)
    }

    fn apply_retention_for_project_unlocked(&self, project_id: &str) -> CoreResult<GcReport> {
        let (project, checkpoints) = {
            let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
            let project = db.get_project(project_id)?;
            let cps = db.list_checkpoints(project_id, None, None, Some(10_000))?;
            (project, cps)
        };

        if checkpoints.len() <= 1 {
            return Ok(GcReport {
                removed_checkpoints: 0,
                removed_objects: 0,
                reclaimed_bytes: 0,
            });
        }

        let now = Utc::now();
        let cutoff = now - Duration::days(project.retention_days as i64);
        let mut to_remove: Vec<String> = Vec::new();

        // Checkpoints are ordered newest-first. Always keep index 0 (latest).
        for (idx, cp) in checkpoints.iter().enumerate() {
            if idx == 0 {
                continue;
            }
            let over_count = idx >= project.max_checkpoints as usize;
            let expired = DateTime::parse_from_rfc3339(&cp.created_at)
                .map(|dt| dt.with_timezone(&Utc) < cutoff)
                .unwrap_or(false);

            if over_count || expired {
                to_remove.push(cp.id.clone());
            }
        }

        let mut removed_checkpoints = 0u64;
        if !to_remove.is_empty() {
            let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
            for id in &to_remove {
                if db.delete_checkpoint(id).is_ok() {
                    removed_checkpoints += 1;
                }
            }
        }

        // Enforce storage limit (max_storage_mb) by pruning oldest auto/watcher checkpoints first
        let max_bytes = project.max_storage_mb.saturating_mul(1024 * 1024);
        loop {
            let (stored_bytes, remaining) = {
                let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
                let stored = db.project_unique_stored_bytes(project_id)?;
                let rem = db.list_checkpoints(project_id, None, None, Some(10_000))?;
                (stored, rem)
            };

            if stored_bytes <= max_bytes || remaining.len() <= 1 {
                break;
            }

            // Pick oldest removable checkpoint (preferring auto/watcher over manual/pre_restore)
            let candidate = remaining
                .iter()
                .skip(1)
                .rev()
                .find(|c| matches!(c.trigger_type, TriggerType::Auto | TriggerType::Watcher))
                .or_else(|| remaining.last());

            if let Some(cp) = candidate {
                if cp.id == remaining[0].id {
                    break;
                }
                let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
                db.delete_checkpoint(&cp.id)?;
                removed_checkpoints += 1;
            } else {
                break;
            }
        }

        let mut gc = self.run_gc()?;
        gc.removed_checkpoints = removed_checkpoints;
        Ok(gc)
    }

    pub fn run_gc(&self) -> CoreResult<GcReport> {
        let orphan_hashes = {
            let mut db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
            db.collect_unreferenced_objects()?
        };

        let mut removed_objects = 0u64;
        let mut reclaimed_bytes = 0u64;
        for hash in orphan_hashes {
            match self.cas.delete_object(&hash) {
                Ok(size) => {
                    removed_objects += 1;
                    reclaimed_bytes += size;
                }
                Err(_) => {}
            }
        }

        Ok(GcReport {
            removed_checkpoints: 0,
            removed_objects,
            reclaimed_bytes,
        })
    }

    pub fn get_storage_stats(&self) -> CoreResult<StorageStats> {
        let (settings, projects, object_count, total_stored_bytes, total_logical_bytes) = {
            let db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
            let settings = db.get_settings()?;
            let projects = db.list_projects()?;
            let (obj_count, stored, _raw_unique) = db.cas_totals()?;
            let logical = db.total_logical_bytes()?;
            (settings, projects, obj_count, stored, logical)
        };

        let available_disk_bytes = fs4::available_space(self.cas.objects_dir()).unwrap_or(0);
        let min_free_disk_bytes = settings.min_free_disk_mb.saturating_mul(1024 * 1024);

        let dedup_savings_bytes = total_logical_bytes.saturating_sub(total_stored_bytes);
        let dedup_savings_percent = if total_logical_bytes > 0 {
            (dedup_savings_bytes as f64 / total_logical_bytes as f64) * 100.0
        } else {
            0.0
        };

        let project_breakdowns = projects
            .into_iter()
            .map(|p| StorageProjectBreakdown {
                project_id: p.id,
                project_name: p.name,
                checkpoint_count: p.checkpoint_count,
                logical_size_bytes: p.logical_size_bytes,
                stored_size_bytes: p.stored_size_bytes,
                max_storage_mb: p.max_storage_mb,
            })
            .collect();

        Ok(StorageStats {
            data_dir: self.data_dir.to_string_lossy().to_string(),
            total_stored_bytes,
            total_logical_bytes,
            object_count,
            dedup_savings_bytes,
            dedup_savings_percent,
            available_disk_bytes,
            min_free_disk_bytes,
            disk_space_low: available_disk_bytes < min_free_disk_bytes,
            projects: project_breakdowns,
        })
    }

    /// Scans the live workspace read-only without modifying CAS or database,
    /// reusing hashes from `hint_files` when `(size, mtime)` match for high speed.
    fn scan_workspace_readonly(
        &self,
        project: &Project,
        settings: &GlobalSettings,
        hint_files: &HashMap<String, CheckpointFile>,
    ) -> CoreResult<HashMap<String, CheckpointFile>> {
        let root_path = validate_project_root(Path::new(&project.root_path))?;
        let matcher = IgnoreMatcher::build(
            &root_path,
            &settings.global_excludes,
            &project.custom_excludes,
            project.respect_gitignore,
            project.max_file_size_mb,
        )?;
        let git_info = if project.git_integration {
            inspect_git_readonly(&root_path)
        } else {
            Default::default()
        };

        let mut map = HashMap::new();
        let mut walker = WalkDir::new(&root_path).follow_links(false).into_iter();
        while let Some(entry_res) = walker.next() {
            let entry = match entry_res {
                Ok(e) => e,
                Err(_) => continue,
            };
            if entry.depth() == 0 {
                continue;
            }
            if entry.file_type().is_symlink() {
                if entry.file_type().is_dir() {
                    walker.skip_current_dir();
                }
                continue;
            }
            let rel_path = match path_to_safe_rel(&root_path, entry.path()) {
                Ok(r) => r,
                Err(_) => {
                    if entry.file_type().is_dir() {
                        walker.skip_current_dir();
                    }
                    continue;
                }
            };
            let is_dir = entry.file_type().is_dir();
            if matcher.is_excluded(&rel_path, is_dir) {
                if is_dir {
                    walker.skip_current_dir();
                }
                continue;
            }
            if is_dir || !entry.file_type().is_file() {
                continue;
            }
            let meta = match fs::symlink_metadata(entry.path()) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let size = meta.len();
            if matcher.exceeds_max_file_size(size) {
                continue;
            }
            let (mtime_secs, mtime_nanos) = extract_mtime(&meta);
            let perms = extract_permissions(&meta);
            let git_status = if git_info.is_git_repo {
                git_info
                    .file_statuses
                    .get(&rel_path)
                    .cloned()
                    .unwrap_or_else(|| "tracked".to_string())
            } else {
                "none".to_string()
            };

            let (object_hash, is_binary) = if let Some(hint) = hint_files.get(&rel_path) {
                if hint.size_bytes == size
                    && hint.mtime_secs == mtime_secs
                    && hint.mtime_nanos == mtime_nanos
                {
                    (hint.object_hash.clone(), hint.is_binary)
                } else {
                    match fs::read(entry.path()) {
                        Ok(bytes) => (
                            blake3::hash(&bytes).to_hex().to_string(),
                            is_probably_binary(&bytes),
                        ),
                        Err(_) => continue,
                    }
                }
            } else {
                match fs::read(entry.path()) {
                    Ok(bytes) => (
                        blake3::hash(&bytes).to_hex().to_string(),
                        is_probably_binary(&bytes),
                    ),
                    Err(_) => continue,
                }
            };

            map.insert(
                rel_path.clone(),
                CheckpointFile {
                    rel_path,
                    object_hash,
                    size_bytes: size,
                    mtime_secs,
                    mtime_nanos,
                    permissions: perms,
                    is_binary,
                    git_status,
                },
            );
        }

        Ok(map)
    }

    /// Helper used by tests to inspect the internal SQLite connection for simulating crash states.
    pub fn with_db_mut<F, R>(&self, f: F) -> CoreResult<R>
    where
        F: FnOnce(&mut Database) -> CoreResult<R>,
    {
        let mut db = self.db.lock().map_err(|_| CoreError::LockPoisoned)?;
        f(&mut db)
    }
}

fn default_checkpoint_label(trigger: TriggerType) -> String {
    let time_str = Utc::now().format("%H:%M:%S UTC");
    match trigger {
        TriggerType::Manual => format!("Manual Checkpoint ({})", time_str),
        TriggerType::Auto => format!("Scheduled Checkpoint ({})", time_str),
        TriggerType::Watcher => format!("Auto Checkpoint ({})", time_str),
        TriggerType::PreRestore => format!("Pre-Restore Safety ({})", time_str),
    }
}

fn extract_mtime(meta: &fs::Metadata) -> (i64, u32) {
    match meta.modified() {
        Ok(sys) => match sys.duration_since(UNIX_EPOCH) {
            Ok(d) => (d.as_secs() as i64, d.subsec_nanos()),
            Err(_) => (0, 0),
        },
        Err(_) => (0, 0),
    }
}

fn extract_permissions(meta: &fs::Metadata) -> u32 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode()
    }
    #[cfg(not(unix))]
    {
        if meta.permissions().readonly() {
            0o444
        } else {
            0o644
        }
    }
}

/// Reads a file while checking that its metadata did not change mid-read (retries up to 2 times).
fn read_file_stably(path: &Path) -> CoreResult<(Vec<u8>, u64, i64, u32)> {
    for attempt in 0..3 {
        let before = fs::symlink_metadata(path)?;
        let (b_sec, b_nano) = extract_mtime(&before);
        let b_len = before.len();
        let data = fs::read(path)?;
        let after = fs::symlink_metadata(path)?;
        let (a_sec, a_nano) = extract_mtime(&after);
        let a_len = after.len();

        if b_len == a_len && b_sec == a_sec && b_nano == a_nano && data.len() as u64 == a_len {
            return Ok((data, a_len, a_sec, a_nano));
        }
        if attempt == 2 {
            return Ok((data, a_len, a_sec, a_nano));
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    unreachable!()
}

pub fn compute_tree_hash(files: &[CheckpointFile]) -> String {
    let mut sorted: Vec<&CheckpointFile> = files.iter().collect();
    sorted.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    let mut hasher = blake3::Hasher::new();
    for f in sorted {
        hasher.update(f.rel_path.as_bytes());
        hasher.update(b":");
        hasher.update(f.object_hash.as_bytes());
        hasher.update(b"\n");
    }
    hasher.finalize().to_hex().to_string()
}

/// Computes a deterministic fingerprint over the current state of affected files to detect
/// concurrent edits between `preview_restore` and `execute_restore`.
fn compute_workspace_fingerprint(root: &Path, affected_paths: &[String]) -> CoreResult<String> {
    let mut hasher = blake3::Hasher::new();
    for rel in affected_paths {
        let full = resolve_safe_project_path(root, rel)?;
        hasher.update(rel.as_bytes());
        hasher.update(b":");
        match fs::symlink_metadata(&full) {
            Ok(meta) if meta.is_file() => {
                let (s, n) = extract_mtime(&meta);
                // Also include fast content hash for files <= 1 MiB so same-millisecond edits are caught!
                hasher.update(format!("F:{}:{}:{}", meta.len(), s, n).as_bytes());
                if meta.len() <= 1024 * 1024 {
                    if let Ok(bytes) = fs::read(&full) {
                        hasher.update(b":");
                        hasher.update(blake3::hash(&bytes).as_bytes());
                    }
                }
            }
            Ok(_) => {
                hasher.update(b"OTHER");
            }
            Err(_) => {
                hasher.update(b"MISSING");
            }
        }
        hasher.update(b"\n");
    }
    Ok(hasher.finalize().to_hex().to_string())
}
