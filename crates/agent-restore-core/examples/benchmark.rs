use std::fs;
use std::path::Path;
use std::time::Instant;

use agent_restore_core::{
    AgentRestoreEngine, CreateCheckpointOptions, CreateProjectRequest, ExecuteRestoreRequest,
    TriggerType,
};
use tempfile::TempDir;

fn write_tree(root: &Path, count: usize, file_size_approx: usize) {
    let pad = "    // Deterministic source line for realistic compression and diffing.\n";
    let repeats = (file_size_approx / pad.len()).max(1);
    for i in 0..count {
        let dir = root.join(format!("src/module_{}", i / 50));
        fs::create_dir_all(&dir).unwrap();
        let mut content = format!("export const MODULE_ID = {};\n", i);
        for r in 0..repeats {
            content.push_str(&format!("export function fn_{}_{}(x: number) {{ return x + {}; }}\n", i, r, (i + r) % 97));
            content.push_str(pad);
        }
        fs::write(dir.join(format!("file_{}.ts", i)), content).unwrap();
    }
}

fn bench_project(file_count: usize, approx_bytes: usize) {
    let data_dir = TempDir::new().unwrap();
    let proj_dir = TempDir::new().unwrap();
    write_tree(proj_dir.path(), file_count, approx_bytes);

    let engine = AgentRestoreEngine::open(data_dir.path()).unwrap();

    // 1. Initial full baseline checkpoint (via add_project)
    let t0 = Instant::now();
    let project = engine
        .add_project(CreateProjectRequest {
            name: Some(format!("Bench {}", file_count)),
            root_path: proj_dir.path().to_string_lossy().into(),
            auto_backup: Some(true),
            interval_secs: None,
            debounce_ms: None,
            max_storage_mb: Some(4096),
            max_checkpoints: Some(50),
            retention_days: Some(30),
            custom_excludes: None,
            respect_gitignore: Some(false),
            git_integration: Some(false),
            max_file_size_mb: None,
        })
        .unwrap();
    let initial_ms = t0.elapsed().as_secs_f64() * 1000.0;

    let baseline_id = engine
        .list_checkpoints(&project.id, None, None, None)
        .unwrap()[0]
        .id
        .clone();

    // 2. Incremental checkpoint with 10 modified files
    for i in 0..10 {
        let p = proj_dir
            .path()
            .join(format!("src/module_0/file_{}.ts", i));
        fs::write(p, format!("export const MODIFIED = {};\n", i)).unwrap();
    }
    let t1 = Instant::now();
    let inc = engine
        .create_checkpoint(CreateCheckpointOptions {
            project_id: project.id.clone(),
            label: Some("Incremental 10".into()),
            note: None,
            trigger_type: TriggerType::Watcher,
            force_if_unchanged: false,
        })
        .unwrap()
        .checkpoint
        .unwrap();
    let incremental_ms = t1.elapsed().as_secs_f64() * 1000.0;

    // 3. No-op incremental scan (0 changed files)
    let t_noop = Instant::now();
    let noop = engine
        .create_checkpoint(CreateCheckpointOptions {
            project_id: project.id.clone(),
            label: None,
            note: None,
            trigger_type: TriggerType::Auto,
            force_if_unchanged: false,
        })
        .unwrap();
    let noop_ms = t_noop.elapsed().as_secs_f64() * 1000.0;
    assert!(noop.skipped_unchanged);

    // 4. Side-by-side diff generation
    let t_diff = Instant::now();
    let _diff = engine
        .get_file_diff(&project.id, &inc.id, "src/module_0/file_0.ts", None, false)
        .unwrap();
    let diff_ms = t_diff.elapsed().as_secs_f64() * 1000.0;

    // 5. Restore 10 modified files back to baseline (includes automatic pre-restore checkpoint)
    let preview = engine
        .preview_restore(&project.id, &baseline_id, None, false)
        .unwrap();
    let t_restore = Instant::now();
    let restore_res = engine
        .execute_restore(ExecuteRestoreRequest {
            project_id: project.id.clone(),
            checkpoint_id: baseline_id,
            selected_paths: None,
            delete_extra_files: false,
            expected_fingerprint: Some(preview.workspace_fingerprint),
            confirmed: true,
        })
        .unwrap();
    let restore_ms = t_restore.elapsed().as_secs_f64() * 1000.0;

    // 6. Full BLAKE3 integrity verification
    let t_verify = Instant::now();
    let verify = engine.verify_integrity(Some(&project.id)).unwrap();
    let verify_ms = t_verify.elapsed().as_secs_f64() * 1000.0;
    assert!(verify.healthy);

    let stats = engine.get_storage_stats().unwrap();

    println!(
        "=== Benchmark: {} files (~{} KB/file) ===",
        file_count,
        approx_bytes / 1024
    );
    println!("  Initial full checkpoint:    {:8.1} ms", initial_ms);
    println!("  Incremental (10 modified):  {:8.1} ms", incremental_ms);
    println!("  Incremental no-op scan:     {:8.1} ms", noop_ms);
    println!("  Side-by-side file diff:     {:8.2} ms", diff_ms);
    println!(
        "  Restore ({} files + safety): {:8.1} ms",
        restore_res.restored_count, restore_ms
    );
    println!(
        "  Verify all ({} objects):   {:8.1} ms",
        verify.checked_objects, verify_ms
    );
    println!(
        "  Logical total:              {:8.2} MB",
        stats.total_logical_bytes as f64 / (1024.0 * 1024.0)
    );
    println!(
        "  Actual stored (CAS+Zstd):   {:8.2} MB ({:.1}% savings)",
        stats.total_stored_bytes as f64 / (1024.0 * 1024.0),
        stats.dedup_savings_percent
    );
    println!();
}

fn main() {
    println!("Agent Restore Core Performance Benchmark\n");
    bench_project(500, 4096);
    bench_project(2_000, 4096);
    bench_project(5_000, 2048);
}
