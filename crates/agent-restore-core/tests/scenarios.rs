use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use agent_restore_core::{
    AgentRestoreEngine, CoreError, CreateCheckpointOptions, CreateProjectRequest,
    ExecuteRestoreRequest, MonitorService, TriggerType, UpdateProjectRequest,
};
use tempfile::TempDir;

fn write_text(root: &Path, rel: &str, content: &str) {
    let full = root.join(rel);
    if let Some(p) = full.parent() {
        fs::create_dir_all(p).unwrap();
    }
    fs::write(full, content).unwrap();
}

fn setup_env() -> (TempDir, TempDir, AgentRestoreEngine) {
    let data_dir = TempDir::new().unwrap();
    let project_dir = TempDir::new().unwrap();
    let engine = AgentRestoreEngine::open(data_dir.path()).unwrap();
    (data_dir, project_dir, engine)
}

#[test]
fn scenario_01_accidental_file_deletion() {
    let (_data, proj_dir, engine) = setup_env();
    write_text(proj_dir.path(), "src/main.rs", "fn main() { println!(\"ok\"); }\n");
    write_text(proj_dir.path(), "src/config.rs", "pub const PORT: u16 = 8080;\n");
    write_text(proj_dir.path(), "README.md", "# Demo\n");

    let project = engine
        .add_project(CreateProjectRequest {
            name: Some("Deletion Test".into()),
            root_path: proj_dir.path().to_string_lossy().into(),
            auto_backup: Some(true),
            interval_secs: None,
            debounce_ms: None,
            max_storage_mb: None,
            max_checkpoints: None,
            retention_days: None,
            custom_excludes: None,
            respect_gitignore: Some(true),
            git_integration: Some(false),
            max_file_size_mb: None,
        })
        .unwrap();

    let cps = engine.list_checkpoints(&project.id, None, None, None).unwrap();
    assert_eq!(cps.len(), 1);
    let baseline = &cps[0];

    // Simulate AI agent accidentally deleting two critical files
    fs::remove_file(proj_dir.path().join("src/main.rs")).unwrap();
    fs::remove_file(proj_dir.path().join("src/config.rs")).unwrap();
    assert!(!proj_dir.path().join("src/main.rs").exists());

    // Preview restore shows exactly 2 deleted files to recreate
    let preview = engine
        .preview_restore(&project.id, &baseline.id, None, false)
        .unwrap();
    assert!(preview.integrity_ok);
    assert_eq!(preview.files_to_recreate.len(), 2);
    assert_eq!(preview.files_to_overwrite.len(), 0);

    // Must reject unconfirmed restore
    let err = engine
        .execute_restore(ExecuteRestoreRequest {
            project_id: project.id.clone(),
            checkpoint_id: baseline.id.clone(),
            selected_paths: None,
            delete_extra_files: false,
            expected_fingerprint: Some(preview.workspace_fingerprint.clone()),
            confirmed: false,
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::ConfirmationRequired));

    // Execute confirmed restore
    let res = engine
        .execute_restore(ExecuteRestoreRequest {
            project_id: project.id.clone(),
            checkpoint_id: baseline.id.clone(),
            selected_paths: None,
            delete_extra_files: false,
            expected_fingerprint: Some(preview.workspace_fingerprint),
            confirmed: true,
        })
        .unwrap();

    assert_eq!(res.restored_count, 2);
    assert_eq!(res.pre_restore_checkpoint.trigger_type, TriggerType::PreRestore);
    assert_eq!(
        fs::read_to_string(proj_dir.path().join("src/main.rs")).unwrap(),
        "fn main() { println!(\"ok\"); }\n"
    );
    assert_eq!(
        fs::read_to_string(proj_dir.path().join("src/config.rs")).unwrap(),
        "pub const PORT: u16 = 8080;\n"
    );
}

#[test]
fn scenario_02_broken_ai_generated_code_and_individual_file_restore() {
    let (_data, proj_dir, engine) = setup_env();
    write_text(
        proj_dir.path(),
        "src/auth.ts",
        "export function verifyToken(token: string): boolean {\n  return token.startsWith('valid_');\n}\n",
    );
    write_text(
        proj_dir.path(),
        "src/notes.ts",
        "export const NOTE = 'keep my new edits';\n",
    );

    let project = engine
        .add_project(CreateProjectRequest {
            name: Some("AI Broken Code".into()),
            root_path: proj_dir.path().to_string_lossy().into(),
            auto_backup: None,
            interval_secs: None,
            debounce_ms: None,
            max_storage_mb: None,
            max_checkpoints: None,
            retention_days: None,
            custom_excludes: None,
            respect_gitignore: None,
            git_integration: Some(false),
            max_file_size_mb: None,
        })
        .unwrap();

    let baseline_id = engine
        .list_checkpoints(&project.id, None, None, None)
        .unwrap()[0]
        .id
        .clone();

    // Simulate AI introducing a critical bug in auth.ts while developer also updates notes.ts
    write_text(
        proj_dir.path(),
        "src/auth.ts",
        "export function verifyToken(_token: string): boolean {\n  return true; // BROKEN BY AI\n}\n",
    );
    write_text(
        proj_dir.path(),
        "src/notes.ts",
        "export const NOTE = 'updated by developer';\n",
    );

    // Side-by-side diff against workspace detects exact change
    let diff = engine
        .get_file_diff(&project.id, &baseline_id, "src/auth.ts", None, true)
        .unwrap();
    assert!(!diff.is_binary);
    assert!(diff.insertions > 0 && diff.deletions > 0);

    // Restore ONLY src/auth.ts
    let preview = engine
        .preview_restore(
            &project.id,
            &baseline_id,
            Some(vec!["src/auth.ts".into()]),
            false,
        )
        .unwrap();
    assert!(!preview.is_full_restore);
    assert_eq!(preview.files_to_overwrite.len(), 1);

    engine
        .execute_restore(ExecuteRestoreRequest {
            project_id: project.id.clone(),
            checkpoint_id: baseline_id,
            selected_paths: Some(vec!["src/auth.ts".into()]),
            delete_extra_files: false,
            expected_fingerprint: Some(preview.workspace_fingerprint),
            confirmed: true,
        })
        .unwrap();

    assert!(fs::read_to_string(proj_dir.path().join("src/auth.ts"))
        .unwrap()
        .contains("token.startsWith('valid_')"));
    // Developer's notes.ts was untouched!
    assert_eq!(
        fs::read_to_string(proj_dir.path().join("src/notes.ts")).unwrap(),
        "export const NOTE = 'updated by developer';\n"
    );
}

#[test]
fn scenario_03_mass_file_modification_and_full_rollback() {
    let (_data, proj_dir, engine) = setup_env();
    for i in 0..150 {
        write_text(
            proj_dir.path(),
            &format!("src/mod_{}/file_{}.rs", i / 15, i),
            &format!("pub fn f_{}() -> usize {{ {} }}\n", i, i),
        );
    }

    let project = engine
        .add_project(CreateProjectRequest {
            name: Some("Mass Edit".into()),
            root_path: proj_dir.path().to_string_lossy().into(),
            auto_backup: None,
            interval_secs: None,
            debounce_ms: None,
            max_storage_mb: None,
            max_checkpoints: None,
            retention_days: None,
            custom_excludes: None,
            respect_gitignore: None,
            git_integration: Some(false),
            max_file_size_mb: None,
        })
        .unwrap();

    let baseline = engine
        .list_checkpoints(&project.id, None, None, None)
        .unwrap()[0]
        .clone();
    assert_eq!(baseline.file_count, 150);

    // Mass break: modify 100, delete 30, create 20 junk files
    for i in 0..100 {
        write_text(
            proj_dir.path(),
            &format!("src/mod_{}/file_{}.rs", i / 15, i),
            "// CORRUPTED MASS REFACTOR\n",
        );
    }
    for i in 100..130 {
        fs::remove_file(proj_dir.path().join(format!("src/mod_{}/file_{}.rs", i / 15, i))).unwrap();
    }
    for j in 0..20 {
        write_text(
            proj_dir.path(),
            &format!("src/junk_{}.rs", j),
            "// unwanted generated file\n",
        );
    }

    let preview = engine
        .preview_restore(&project.id, &baseline.id, None, true)
        .unwrap();
    assert_eq!(preview.files_to_overwrite.len(), 100);
    assert_eq!(preview.files_to_recreate.len(), 30);
    assert_eq!(preview.extra_files_to_delete.len(), 20);
    assert_eq!(preview.unchanged_count, 20);

    let res = engine
        .execute_restore(ExecuteRestoreRequest {
            project_id: project.id.clone(),
            checkpoint_id: baseline.id.clone(),
            selected_paths: None,
            delete_extra_files: true,
            expected_fingerprint: Some(preview.workspace_fingerprint),
            confirmed: true,
        })
        .unwrap();

    assert_eq!(res.restored_count, 130);
    assert_eq!(res.deleted_count, 20);

    // Every single original file matches and junk files are gone
    for i in 0..150 {
        let content = fs::read_to_string(
            proj_dir
                .path()
                .join(format!("src/mod_{}/file_{}.rs", i / 15, i)),
        )
        .unwrap();
        assert_eq!(content, format!("pub fn f_{}() -> usize {{ {} }}\n", i, i));
    }
    for j in 0..20 {
        assert!(!proj_dir.path().join(format!("src/junk_{}.rs", j)).exists());
    }
}

#[test]
fn scenario_04_untracked_files_secrets_exclusion_and_git_immutability() {
    let (_data, proj_dir, engine) = setup_env();

    // Initialize a real git repository in isolated TempDir
    let git_ok = Command::new("git")
        .current_dir(proj_dir.path())
        .args(["init"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    assert!(git_ok, "git must be available for git immutability test");

    write_text(proj_dir.path(), "src/app.ts", "export const v = 1;\n");
    let _ = Command::new("git")
        .current_dir(proj_dir.path())
        .args(["config", "user.email", "test@agentrestore.local"])
        .output();
    let _ = Command::new("git")
        .current_dir(proj_dir.path())
        .args(["config", "user.name", "Tester"])
        .output();
    let _ = Command::new("git")
        .current_dir(proj_dir.path())
        .args(["add", "src/app.ts"])
        .output();
    let _ = Command::new("git")
        .current_dir(proj_dir.path())
        .args(["commit", "-m", "initial"])
        .output();

    // Add eligible untracked file + sensitive secrets + build cache
    write_text(
        proj_dir.path(),
        "src/untracked_experiment.ts",
        "export const draft = 'important untracked work';\n",
    );
    write_text(proj_dir.path(), ".env", "API_SECRET=super_secret_token\n");
    write_text(proj_dir.path(), ".env.example", "API_SECRET=changeme\n");
    write_text(proj_dir.path(), "certs/private.pem", "-----BEGIN PRIVATE KEY-----");
    write_text(proj_dir.path(), "node_modules/pkg/index.js", "module.exports = 1;");

    // Snapshot .git/HEAD and .git/config bytes before Agent Restore touches anything
    let git_head_before = fs::read(proj_dir.path().join(".git/HEAD")).unwrap();
    let git_config_before = fs::read(proj_dir.path().join(".git/config")).unwrap();

    let project = engine
        .add_project(CreateProjectRequest {
            name: Some("Git Untracked Test".into()),
            root_path: proj_dir.path().to_string_lossy().into(),
            auto_backup: None,
            interval_secs: None,
            debounce_ms: None,
            max_storage_mb: None,
            max_checkpoints: None,
            retention_days: None,
            custom_excludes: None,
            respect_gitignore: Some(true),
            git_integration: Some(true),
            max_file_size_mb: None,
        })
        .unwrap();

    let cp = engine
        .list_checkpoints(&project.id, None, None, None)
        .unwrap()[0]
        .clone();
    assert!(cp.git_branch.is_some());
    assert!(cp.git_commit.is_some());

    let files = engine.get_checkpoint_files(&cp.id).unwrap();
    let paths: Vec<&str> = files.iter().map(|f| f.rel_path.as_str()).collect();

    // Eligible untracked file & .env.example ARE backed up
    assert!(paths.contains(&"src/app.ts"));
    assert!(paths.contains(&"src/untracked_experiment.ts"));
    assert!(paths.contains(&".env.example"));
    // Secrets, caches, and .git are EXCLUDED
    assert!(!paths.contains(&".env"));
    assert!(!paths.contains(&"certs/private.pem"));
    assert!(!paths.contains(&"node_modules/pkg/index.js"));
    assert!(!paths.iter().any(|p| p.contains(".git")));

    let untracked = files
        .iter()
        .find(|f| f.rel_path == "src/untracked_experiment.ts")
        .unwrap();
    assert_eq!(untracked.git_status, "untracked");

    // Delete the untracked file and restore it
    fs::remove_file(proj_dir.path().join("src/untracked_experiment.ts")).unwrap();
    let preview = engine
        .preview_restore(
            &project.id,
            &cp.id,
            Some(vec!["src/untracked_experiment.ts".into()]),
            false,
        )
        .unwrap();
    engine
        .execute_restore(ExecuteRestoreRequest {
            project_id: project.id.clone(),
            checkpoint_id: cp.id.clone(),
            selected_paths: Some(vec!["src/untracked_experiment.ts".into()]),
            delete_extra_files: false,
            expected_fingerprint: Some(preview.workspace_fingerprint),
            confirmed: true,
        })
        .unwrap();

    assert_eq!(
        fs::read_to_string(proj_dir.path().join("src/untracked_experiment.ts")).unwrap(),
        "export const draft = 'important untracked work';\n"
    );

    // .git remains 100% untouched!
    assert_eq!(fs::read(proj_dir.path().join(".git/HEAD")).unwrap(), git_head_before);
    assert_eq!(fs::read(proj_dir.path().join(".git/config")).unwrap(), git_config_before);
}

#[test]
fn scenario_05_crash_during_backup_recovery() {
    let data_dir = TempDir::new().unwrap();
    let proj_dir = TempDir::new().unwrap();
    write_text(proj_dir.path(), "index.ts", "console.log('healthy');\n");

    let project_id = {
        let engine = AgentRestoreEngine::open(data_dir.path()).unwrap();
        let project = engine
            .add_project(CreateProjectRequest {
                name: Some("Crash Backup Test".into()),
                root_path: proj_dir.path().to_string_lossy().into(),
                auto_backup: None,
                interval_secs: None,
                debounce_ms: None,
                max_storage_mb: None,
                max_checkpoints: None,
                retention_days: None,
                custom_excludes: None,
                respect_gitignore: None,
                git_integration: Some(false),
                max_file_size_mb: None,
            })
            .unwrap();

        // Simulate abrupt app crash mid-backup: dangling in_progress checkpoint + orphaned CAS temp file
        engine
            .with_db_mut(|db| {
                db.begin_checkpoint_record(
                    "crashed-cp-123",
                    &project.id,
                    "Crashed Backup",
                    "",
                    TriggerType::Auto,
                    None,
                    "2026-11-25T00:00:00Z",
                )
            })
            .unwrap();
        fs::write(engine.cas().tmp_dir().join("obj-orphan.tmp"), b"partial data").unwrap();
        project.id
    };

    // Reopen engine (simulating restart after crash)
    let recovered = AgentRestoreEngine::open(data_dir.path()).unwrap();
    let report = recovered.get_crash_recovery_report().unwrap();
    assert_eq!(report.cleaned_incomplete_checkpoints, 1);
    assert!(report.cleaned_temp_files >= 1);

    // Only the clean completed checkpoint remains, and integrity is healthy
    let cps = recovered
        .list_checkpoints(&project_id, None, None, None)
        .unwrap();
    assert_eq!(cps.len(), 1);
    assert!(recovered.verify_integrity(Some(&project_id)).unwrap().healthy);
}

#[test]
fn scenario_06_interrupted_restoration_recovery() {
    let data_dir = TempDir::new().unwrap();
    let proj_dir = TempDir::new().unwrap();
    write_text(proj_dir.path(), "app.py", "print('v1')\n");

    let (project_id, pre_restore_id) = {
        let engine = AgentRestoreEngine::open(data_dir.path()).unwrap();
        let project = engine
            .add_project(CreateProjectRequest {
                name: Some("Interrupted Restore".into()),
                root_path: proj_dir.path().to_string_lossy().into(),
                auto_backup: None,
                interval_secs: None,
                debounce_ms: None,
                max_storage_mb: None,
                max_checkpoints: None,
                retention_days: None,
                custom_excludes: None,
                respect_gitignore: None,
                git_integration: Some(false),
                max_file_size_mb: None,
            })
            .unwrap();

        write_text(proj_dir.path(), "app.py", "print('v2-good')\n");
        let pre = engine
            .create_checkpoint(CreateCheckpointOptions {
                project_id: project.id.clone(),
                label: Some("Pre-Restore Safety".into()),
                note: None,
                trigger_type: TriggerType::PreRestore,
                force_if_unchanged: true,
            })
            .unwrap()
            .checkpoint
            .unwrap();

        // Simulate crash mid-restore: in_progress restore op + leftover temp staging file + partial edit
        engine
            .with_db_mut(|db| {
                db.start_operation(
                    &project.id,
                    "restore",
                    &serde_json::json!({
                        "preRestoreCheckpointId": pre.id
                    }),
                )?;
                Ok(())
            })
            .unwrap();
        fs::write(proj_dir.path().join(".agentrestore-tmp-partial"), b"half written").unwrap();
        write_text(proj_dir.path(), "app.py", "print('half-restored')\n");

        (project.id, pre.id)
    };

    // Restart engine
    let recovered = AgentRestoreEngine::open(data_dir.path()).unwrap();
    let report = recovered.get_crash_recovery_report().unwrap();
    assert_eq!(report.interrupted_restores.len(), 1);
    assert_eq!(
        report.interrupted_restores[0].recovery_checkpoint_id.as_deref(),
        Some(pre_restore_id.as_str())
    );
    // Staging temp file was automatically cleaned up
    assert!(!proj_dir.path().join(".agentrestore-tmp-partial").exists());

    // User can immediately recover using the pre_restore checkpoint
    let preview = recovered
        .preview_restore(&project_id, &pre_restore_id, None, false)
        .unwrap();
    recovered
        .execute_restore(ExecuteRestoreRequest {
            project_id: project_id.clone(),
            checkpoint_id: pre_restore_id,
            selected_paths: None,
            delete_extra_files: false,
            expected_fingerprint: Some(preview.workspace_fingerprint),
            confirmed: true,
        })
        .unwrap();

    assert_eq!(
        fs::read_to_string(proj_dir.path().join("app.py")).unwrap(),
        "print('v2-good')\n"
    );
}

#[test]
fn scenario_07_large_project_incremental_and_deduplication() {
    let (_data, proj_dir, engine) = setup_env();
    for i in 0..1_200 {
        write_text(
            proj_dir.path(),
            &format!("packages/pkg_{}/src/file_{}.ts", i / 60, i),
            &format!(
                "// Shared boilerplate header for deduplication testing\nexport const id_{} = {};\nexport function compute(x: number) {{ return x * 2 + {}; }}\n",
                i, i, i
            ),
        );
    }

    let project = engine
        .add_project(CreateProjectRequest {
            name: Some("Large Monorepo".into()),
            root_path: proj_dir.path().to_string_lossy().into(),
            auto_backup: None,
            interval_secs: None,
            debounce_ms: None,
            max_storage_mb: None,
            max_checkpoints: None,
            retention_days: None,
            custom_excludes: None,
            respect_gitignore: None,
            git_integration: Some(false),
            max_file_size_mb: None,
        })
        .unwrap();

    // Modify only 5 files out of 1,200
    for i in 0..5 {
        write_text(
            proj_dir.path(),
            &format!("packages/pkg_0/src/file_{}.ts", i),
            &format!("// Modified file {}\nexport const updated = true;\n", i),
        );
    }

    let inc = engine
        .create_checkpoint(CreateCheckpointOptions {
            project_id: project.id.clone(),
            label: Some("Incremental Update".into()),
            note: None,
            trigger_type: TriggerType::Manual,
            force_if_unchanged: false,
        })
        .unwrap()
        .checkpoint
        .unwrap();

    assert_eq!(inc.file_count, 1_200);
    assert_eq!(inc.modified_count, 5);
    assert_eq!(inc.added_count, 0);
    assert_eq!(inc.deleted_count, 0);

    // Check deduplication savings across the 2 checkpoints
    let stats = engine.get_storage_stats().unwrap();
    assert!(stats.total_stored_bytes < stats.total_logical_bytes / 2);
    assert!(stats.dedup_savings_percent > 50.0);

    let integrity = engine.verify_integrity(Some(&project.id)).unwrap();
    assert!(integrity.healthy);
    assert_eq!(integrity.checked_checkpoints, 2);
}

#[test]
fn scenario_08_insufficient_disk_space_safe_refusal() {
    let (_data, proj_dir, engine) = setup_env();
    write_text(proj_dir.path(), "data.txt", "original content\n");

    let project = engine
        .add_project(CreateProjectRequest {
            name: Some("Disk Guard".into()),
            root_path: proj_dir.path().to_string_lossy().into(),
            auto_backup: None,
            interval_secs: None,
            debounce_ms: None,
            max_storage_mb: None,
            max_checkpoints: None,
            retention_days: None,
            custom_excludes: None,
            respect_gitignore: None,
            git_integration: Some(false),
            max_file_size_mb: None,
        })
        .unwrap();
    let cp_id = engine
        .list_checkpoints(&project.id, None, None, None)
        .unwrap()[0]
        .id
        .clone();

    // Configure an impossibly high minimum free space floor (100 PB)
    let mut settings = engine.get_settings().unwrap();
    settings.min_free_disk_mb = 100_000_000_000;
    engine.save_settings(settings).unwrap();

    write_text(proj_dir.path(), "data.txt", "changed content\n");

    // Backup safely refuses with InsufficientDiskSpace
    let backup_err = engine
        .create_checkpoint(CreateCheckpointOptions {
            project_id: project.id.clone(),
            label: Some("Should Fail".into()),
            note: None,
            trigger_type: TriggerType::Manual,
            force_if_unchanged: true,
        })
        .unwrap_err();
    assert!(matches!(backup_err, CoreError::InsufficientDiskSpace { .. }));

    // Restore also safely refuses without touching user's current file
    let preview = engine.preview_restore(&project.id, &cp_id, None, false).unwrap();
    let restore_err = engine
        .execute_restore(ExecuteRestoreRequest {
            project_id: project.id.clone(),
            checkpoint_id: cp_id,
            selected_paths: None,
            delete_extra_files: false,
            expected_fingerprint: Some(preview.workspace_fingerprint),
            confirmed: true,
        })
        .unwrap_err();
    assert!(matches!(restore_err, CoreError::InsufficientDiskSpace { .. }));
    assert_eq!(
        fs::read_to_string(proj_dir.path().join("data.txt")).unwrap(),
        "changed content\n"
    );
}

#[test]
fn scenario_09_corrupted_snapshot_blocked_before_restore() {
    let (_data, proj_dir, engine) = setup_env();
    write_text(proj_dir.path(), "core.rs", "pub fn critical() -> i32 { 42 }\n");

    let project = engine
        .add_project(CreateProjectRequest {
            name: Some("Corrupt Test".into()),
            root_path: proj_dir.path().to_string_lossy().into(),
            auto_backup: None,
            interval_secs: None,
            debounce_ms: None,
            max_storage_mb: None,
            max_checkpoints: None,
            retention_days: None,
            custom_excludes: None,
            respect_gitignore: None,
            git_integration: Some(false),
            max_file_size_mb: None,
        })
        .unwrap();

    let cp_id = engine
        .list_checkpoints(&project.id, None, None, None)
        .unwrap()[0]
        .id
        .clone();
    let files = engine.get_checkpoint_files(&cp_id).unwrap();
    let obj_path = engine.cas().object_path(&files[0].object_hash).unwrap();

    // Tamper with a byte inside the CAS object file on disk
    let mut raw = fs::read(&obj_path).unwrap();
    let last = raw.len() - 1;
    raw[last] ^= 0xFF;
    fs::write(&obj_path, raw).unwrap();

    // Integrity check catches it
    let integrity = engine.verify_integrity(Some(&project.id)).unwrap();
    assert!(!integrity.healthy);
    assert_eq!(integrity.issues.len(), 1);

    // Modify workspace file and attempt restore -> MUST fail before modifying workspace!
    write_text(proj_dir.path(), "core.rs", "pub fn critical() -> i32 { 999 }\n");
    let preview = engine.preview_restore(&project.id, &cp_id, None, false).unwrap();
    assert!(!preview.integrity_ok);

    let err = engine
        .execute_restore(ExecuteRestoreRequest {
            project_id: project.id.clone(),
            checkpoint_id: cp_id,
            selected_paths: None,
            delete_extra_files: false,
            expected_fingerprint: Some(preview.workspace_fingerprint),
            confirmed: true,
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::CorruptedSnapshot { .. }));

    // Workspace file was NOT overwritten with corrupt bytes
    assert_eq!(
        fs::read_to_string(proj_dir.path().join("core.rs")).unwrap(),
        "pub fn critical() -> i32 { 999 }\n"
    );
}

#[test]
fn scenario_10_concurrent_file_modification_conflict_detection() {
    let (_data, proj_dir, engine) = setup_env();
    write_text(proj_dir.path(), "service.rs", "const V: u32 = 1;\n");

    let project = engine
        .add_project(CreateProjectRequest {
            name: Some("Conflict Test".into()),
            root_path: proj_dir.path().to_string_lossy().into(),
            auto_backup: None,
            interval_secs: None,
            debounce_ms: None,
            max_storage_mb: None,
            max_checkpoints: None,
            retention_days: None,
            custom_excludes: None,
            respect_gitignore: None,
            git_integration: Some(false),
            max_file_size_mb: None,
        })
        .unwrap();
    let cp_id = engine
        .list_checkpoints(&project.id, None, None, None)
        .unwrap()[0]
        .id
        .clone();

    write_text(proj_dir.path(), "service.rs", "const V: u32 = 2;\n");
    let preview = engine.preview_restore(&project.id, &cp_id, None, false).unwrap();

    // Simulate AI agent or editor concurrently modifying the file AFTER preview was shown
    write_text(proj_dir.path(), "service.rs", "const V: u32 = 3; // concurrent edit!\n");

    let err = engine
        .execute_restore(ExecuteRestoreRequest {
            project_id: project.id.clone(),
            checkpoint_id: cp_id.clone(),
            selected_paths: None,
            delete_extra_files: false,
            expected_fingerprint: Some(preview.workspace_fingerprint),
            confirmed: true,
        })
        .unwrap_err();
    assert!(matches!(err, CoreError::ConcurrentModification { .. }));
    // Concurrent edit was NOT silently clobbered!
    assert_eq!(
        fs::read_to_string(proj_dir.path().join("service.rs")).unwrap(),
        "const V: u32 = 3; // concurrent edit!\n"
    );
}

#[test]
fn scenario_11_unauthorized_filesystem_access_blocked() {
    let (_data, proj_dir, engine) = setup_env();
    let outside_dir = TempDir::new().unwrap();
    write_text(outside_dir.path(), "secret.txt", "do not touch");
    write_text(proj_dir.path(), "ok.txt", "inside project");

    let project = engine
        .add_project(CreateProjectRequest {
            name: Some("Security Test".into()),
            root_path: proj_dir.path().to_string_lossy().into(),
            auto_backup: None,
            interval_secs: None,
            debounce_ms: None,
            max_storage_mb: None,
            max_checkpoints: None,
            retention_days: None,
            custom_excludes: None,
            respect_gitignore: None,
            git_integration: Some(false),
            max_file_size_mb: None,
        })
        .unwrap();
    let cp_id = engine
        .list_checkpoints(&project.id, None, None, None)
        .unwrap()[0]
        .id
        .clone();

    // 1. Path traversal attempts blocked
    for bad_path in [
        "../secret.txt",
        "..\\secret.txt",
        "subdir/../../secret.txt",
        "/etc/passwd",
        "C:\\Windows\\win.ini",
        "foo\0bar.txt",
    ] {
        let err = engine
            .preview_restore(&project.id, &cp_id, Some(vec![bad_path.into()]), false)
            .unwrap_err();
        assert!(
            matches!(err, CoreError::PathTraversal(_)),
            "Expected PathTraversal for {}, got {:?}",
            bad_path,
            err
        );
    }

    // 2. .git modification/access blocked
    for git_path in [".git/config", ".git/HEAD", "nested/.git/index"] {
        let err = engine
            .preview_restore(&project.id, &cp_id, Some(vec![git_path.into()]), false)
            .unwrap_err();
        assert!(
            matches!(err, CoreError::ProtectedGitPath(_)),
            "Expected ProtectedGitPath for {}, got {:?}",
            git_path,
            err
        );
    }

    // 3. Cross-project checkpoint isolation
    let project2_dir = TempDir::new().unwrap();
    write_text(project2_dir.path(), "other.txt", "other project");
    let project2 = engine
        .add_project(CreateProjectRequest {
            name: Some("Other Project".into()),
            root_path: project2_dir.path().to_string_lossy().into(),
            auto_backup: None,
            interval_secs: None,
            debounce_ms: None,
            max_storage_mb: None,
            max_checkpoints: None,
            retention_days: None,
            custom_excludes: None,
            respect_gitignore: None,
            git_integration: Some(false),
            max_file_size_mb: None,
        })
        .unwrap();

    let cross_err = engine
        .preview_restore(&project2.id, &cp_id, None, false)
        .unwrap_err();
    assert!(matches!(cross_err, CoreError::UnauthorizedPath(_)));
}

#[test]
fn scenario_12_retention_policy_gc_and_realtime_watcher() {
    let (_data, proj_dir, engine) = setup_env();
    write_text(proj_dir.path(), "main.rs", "fn main() { /* v0 */ }\n");

    let engine = Arc::new(engine);
    let project = engine
        .add_project(CreateProjectRequest {
            name: Some("Watcher & Retention".into()),
            root_path: proj_dir.path().to_string_lossy().into(),
            auto_backup: Some(true),
            interval_secs: Some(600),
            debounce_ms: Some(400),
            max_storage_mb: Some(128),
            max_checkpoints: Some(3),
            retention_days: Some(30),
            custom_excludes: None,
            respect_gitignore: Some(false),
            git_integration: Some(false),
            max_file_size_mb: None,
        })
        .unwrap();

    // Test retention caps checkpoint count to max_checkpoints = 3
    for i in 1..=5 {
        write_text(
            proj_dir.path(),
            "main.rs",
            &format!("fn main() {{ /* v{} */ }}\n", i),
        );
        engine
            .create_checkpoint(CreateCheckpointOptions {
                project_id: project.id.clone(),
                label: Some(format!("Auto {}", i)),
                note: None,
                trigger_type: TriggerType::Auto,
                force_if_unchanged: false,
            })
            .unwrap();
    }

    let cps_after_retention = engine
        .list_checkpoints(&project.id, None, None, None)
        .unwrap();
    assert!(cps_after_retention.len() <= 3);

    // Test live debounced filesystem watcher
    let events = Arc::new(Mutex::new(Vec::<String>::new()));
    let events_clone = events.clone();
    let monitor = MonitorService::start(engine.clone(), move |ev| {
        if ev.kind == "checkpoint_created" {
            events_clone.lock().unwrap().push(ev.checkpoint_id.unwrap_or_default());
        }
    });

    // Increase max_checkpoints so watcher checkpoint is easy to observe
    engine
        .update_project(
            &project.id,
            UpdateProjectRequest {
                name: None,
                enabled: None,
                auto_backup: None,
                interval_secs: None,
                debounce_ms: Some(350),
                max_storage_mb: None,
                max_checkpoints: Some(20),
                retention_days: None,
                custom_excludes: None,
                respect_gitignore: None,
                git_integration: None,
                max_file_size_mb: None,
            },
        )
        .unwrap();
    monitor.reload_projects();
    std::thread::sleep(Duration::from_millis(250));

    write_text(proj_dir.path(), "watcher_added.rs", "pub const WATCHED: bool = true;\n");

    let start = Instant::now();
    let mut got_watcher_cp = false;
    while start.elapsed() < Duration::from_secs(6) {
        if !events.lock().unwrap().is_empty() {
            got_watcher_cp = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    monitor.shutdown();

    assert!(got_watcher_cp, "Watcher should create an automatic debounced checkpoint");
}
