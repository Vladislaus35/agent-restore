use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalSettings {
    pub language: String,
    pub theme: String,
    pub minimize_to_tray: bool,
    pub auto_start_monitoring: bool,
    pub default_interval_secs: u64,
    pub default_debounce_ms: u64,
    pub default_max_storage_mb: u64,
    pub default_max_checkpoints: u32,
    pub default_retention_days: u32,
    pub default_max_file_size_mb: u64,
    pub min_free_disk_mb: u64,
    pub global_excludes: Vec<String>,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            theme: "dark".to_string(),
            minimize_to_tray: true,
            auto_start_monitoring: true,
            default_interval_secs: 180,
            default_debounce_ms: 2500,
            default_max_storage_mb: 2048,
            default_max_checkpoints: 200,
            default_retention_days: 30,
            default_max_file_size_mb: 50,
            min_free_disk_mb: 256,
            global_excludes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub root_path: String,
    pub enabled: bool,
    pub auto_backup: bool,
    pub interval_secs: u64,
    pub debounce_ms: u64,
    pub max_storage_mb: u64,
    pub max_checkpoints: u32,
    pub retention_days: u32,
    pub custom_excludes: Vec<String>,
    pub respect_gitignore: bool,
    pub git_integration: bool,
    pub max_file_size_mb: u64,
    pub created_at: String,
    pub updated_at: String,
    pub last_checkpoint_at: Option<String>,
    pub checkpoint_count: u64,
    pub stored_size_bytes: u64,
    pub logical_size_bytes: u64,
    pub git_branch: Option<String>,
    pub git_commit: Option<String>,
    pub git_dirty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectRequest {
    pub name: Option<String>,
    pub root_path: String,
    pub auto_backup: Option<bool>,
    pub interval_secs: Option<u64>,
    pub debounce_ms: Option<u64>,
    pub max_storage_mb: Option<u64>,
    pub max_checkpoints: Option<u32>,
    pub retention_days: Option<u32>,
    pub custom_excludes: Option<Vec<String>>,
    pub respect_gitignore: Option<bool>,
    pub git_integration: Option<bool>,
    pub max_file_size_mb: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectRequest {
    pub name: Option<String>,
    pub enabled: Option<bool>,
    pub auto_backup: Option<bool>,
    pub interval_secs: Option<u64>,
    pub debounce_ms: Option<u64>,
    pub max_storage_mb: Option<u64>,
    pub max_checkpoints: Option<u32>,
    pub retention_days: Option<u32>,
    pub custom_excludes: Option<Vec<String>>,
    pub respect_gitignore: Option<bool>,
    pub git_integration: Option<bool>,
    pub max_file_size_mb: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerType {
    Manual,
    Auto,
    Watcher,
    PreRestore,
}

impl TriggerType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TriggerType::Manual => "manual",
            TriggerType::Auto => "auto",
            TriggerType::Watcher => "watcher",
            TriggerType::PreRestore => "pre_restore",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "manual" => TriggerType::Manual,
            "auto" => TriggerType::Auto,
            "watcher" => TriggerType::Watcher,
            "pre_restore" => TriggerType::PreRestore,
            _ => TriggerType::Auto,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Checkpoint {
    pub id: String,
    pub project_id: String,
    pub label: String,
    pub note: String,
    pub trigger_type: TriggerType,
    pub status: String,
    pub parent_checkpoint_id: Option<String>,
    pub file_count: u64,
    pub total_size_bytes: u64,
    pub stored_size_bytes: u64,
    pub added_count: u64,
    pub modified_count: u64,
    pub deleted_count: u64,
    pub git_branch: Option<String>,
    pub git_commit: Option<String>,
    pub git_dirty: bool,
    pub tree_hash: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointFile {
    pub rel_path: String,
    pub object_hash: String,
    pub size_bytes: u64,
    pub mtime_secs: i64,
    pub mtime_nanos: u32,
    pub permissions: u32,
    pub is_binary: bool,
    pub git_status: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Unchanged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeEntry {
    pub rel_path: String,
    pub kind: ChangeKind,
    pub old_size: Option<u64>,
    pub new_size: Option<u64>,
    pub is_binary: bool,
    pub git_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointDiffSummary {
    pub base_checkpoint_id: Option<String>,
    pub target_checkpoint_id: Option<String>,
    pub compared_with_workspace: bool,
    pub added_count: u64,
    pub modified_count: u64,
    pub deleted_count: u64,
    pub changes: Vec<FileChangeEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffLineKind {
    Equal,
    Insert,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SideBySideRow {
    pub left_line_no: Option<usize>,
    pub left_text: Option<String>,
    pub left_kind: Option<DiffLineKind>,
    pub right_line_no: Option<usize>,
    pub right_text: Option<String>,
    pub right_kind: Option<DiffLineKind>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiffResult {
    pub rel_path: String,
    pub is_binary: bool,
    pub truncated: bool,
    pub left_label: String,
    pub right_label: String,
    pub insertions: usize,
    pub deletions: usize,
    pub rows: Vec<SideBySideRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCheckpointOptions {
    pub project_id: String,
    pub label: Option<String>,
    pub note: Option<String>,
    pub trigger_type: TriggerType,
    pub force_if_unchanged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCheckpointOutcome {
    pub checkpoint: Option<Checkpoint>,
    pub skipped_unchanged: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreFilePlan {
    pub rel_path: String,
    pub action: String, // "restore_deleted" | "overwrite" | "delete_extra" | "keep_extra"
    pub current_size: Option<u64>,
    pub target_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePreview {
    pub project_id: String,
    pub checkpoint_id: String,
    pub checkpoint_label: String,
    pub is_full_restore: bool,
    pub delete_extra_files: bool,
    pub files_to_recreate: Vec<RestoreFilePlan>,
    pub files_to_overwrite: Vec<RestoreFilePlan>,
    pub extra_files_to_delete: Vec<RestoreFilePlan>,
    pub extra_files_kept: Vec<RestoreFilePlan>,
    pub unchanged_count: u64,
    pub total_bytes_to_write: u64,
    pub workspace_fingerprint: String,
    pub integrity_ok: bool,
    pub corrupted_objects: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteRestoreRequest {
    pub project_id: String,
    pub checkpoint_id: String,
    pub selected_paths: Option<Vec<String>>,
    pub delete_extra_files: bool,
    pub expected_fingerprint: Option<String>,
    pub confirmed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreResult {
    pub pre_restore_checkpoint: Checkpoint,
    pub restored_count: u64,
    pub deleted_count: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageProjectBreakdown {
    pub project_id: String,
    pub project_name: String,
    pub checkpoint_count: u64,
    pub logical_size_bytes: u64,
    pub stored_size_bytes: u64,
    pub max_storage_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageStats {
    pub data_dir: String,
    pub total_stored_bytes: u64,
    pub total_logical_bytes: u64,
    pub object_count: u64,
    pub dedup_savings_bytes: u64,
    pub dedup_savings_percent: f64,
    pub available_disk_bytes: u64,
    pub min_free_disk_bytes: u64,
    pub disk_space_low: bool,
    pub projects: Vec<StorageProjectBreakdown>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GcReport {
    pub removed_checkpoints: u64,
    pub removed_objects: u64,
    pub reclaimed_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrityIssue {
    pub checkpoint_id: String,
    pub rel_path: String,
    pub object_hash: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrityReport {
    pub checked_checkpoints: u64,
    pub checked_objects: u64,
    pub healthy: bool,
    pub issues: Vec<IntegrityIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashRecoveryNotice {
    pub op_id: String,
    pub project_id: String,
    pub op_type: String,
    pub message: String,
    pub recovery_checkpoint_id: Option<String>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashRecoveryReport {
    pub cleaned_incomplete_checkpoints: u64,
    pub cleaned_temp_files: u64,
    pub interrupted_restores: Vec<CrashRecoveryNotice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStatus {
    pub project_id: String,
    pub has_pending_changes: bool,
    pub added_count: u64,
    pub modified_count: u64,
    pub deleted_count: u64,
    pub tracked_file_count: u64,
    pub untracked_file_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitoringEvent {
    pub kind: String, // "status" | "checkpoint_created" | "changes_detected" | "error"
    pub project_id: Option<String>,
    pub state: Option<String>, // "monitoring" | "backing_up" | "paused" | "idle" | "error"
    pub checkpoint_id: Option<String>,
    pub message: Option<String>,
}
