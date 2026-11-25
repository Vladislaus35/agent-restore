use similar::{ChangeTag, TextDiff};
use std::collections::{BTreeSet, HashMap};

use crate::models::{
    ChangeKind, CheckpointDiffSummary, CheckpointFile, DiffLineKind, FileChangeEntry,
    FileDiffResult, SideBySideRow,
};
use crate::security::is_probably_binary;

const MAX_DIFF_BYTES: usize = 2 * 1024 * 1024; // 2 MiB text diff cap for UI responsiveness
const MAX_DIFF_ROWS: usize = 12_000;

/// Compares two file maps (`base` -> `target`) and returns added, modified, and deleted entries.
pub fn diff_file_maps(
    base: &HashMap<String, CheckpointFile>,
    target: &HashMap<String, CheckpointFile>,
    base_id: Option<String>,
    target_id: Option<String>,
    compared_with_workspace: bool,
) -> CheckpointDiffSummary {
    let mut all_paths: BTreeSet<&String> = BTreeSet::new();
    for k in base.keys() {
        all_paths.insert(k);
    }
    for k in target.keys() {
        all_paths.insert(k);
    }

    let mut changes = Vec::new();
    let mut added_count = 0u64;
    let mut modified_count = 0u64;
    let mut deleted_count = 0u64;

    for path in all_paths {
        match (base.get(path), target.get(path)) {
            (None, Some(t)) => {
                added_count += 1;
                changes.push(FileChangeEntry {
                    rel_path: path.clone(),
                    kind: ChangeKind::Added,
                    old_size: None,
                    new_size: Some(t.size_bytes),
                    is_binary: t.is_binary,
                    git_status: t.git_status.clone(),
                });
            }
            (Some(b), None) => {
                deleted_count += 1;
                changes.push(FileChangeEntry {
                    rel_path: path.clone(),
                    kind: ChangeKind::Deleted,
                    old_size: Some(b.size_bytes),
                    new_size: None,
                    is_binary: b.is_binary,
                    git_status: b.git_status.clone(),
                });
            }
            (Some(b), Some(t)) => {
                if b.object_hash != t.object_hash {
                    modified_count += 1;
                    changes.push(FileChangeEntry {
                        rel_path: path.clone(),
                        kind: ChangeKind::Modified,
                        old_size: Some(b.size_bytes),
                        new_size: Some(t.size_bytes),
                        is_binary: b.is_binary || t.is_binary,
                        git_status: t.git_status.clone(),
                    });
                }
            }
            (None, None) => {}
        }
    }

    CheckpointDiffSummary {
        base_checkpoint_id: base_id,
        target_checkpoint_id: target_id,
        compared_with_workspace,
        added_count,
        modified_count,
        deleted_count,
        changes,
    }
}

/// Computes a clean, paired side-by-side diff suitable for code review UI rendering.
pub fn compute_side_by_side_diff(
    rel_path: &str,
    left_bytes: Option<&[u8]>,
    right_bytes: Option<&[u8]>,
    left_label: String,
    right_label: String,
) -> FileDiffResult {
    let left_slice = left_bytes.unwrap_or(&[]);
    let right_slice = right_bytes.unwrap_or(&[]);

    if (!left_slice.is_empty() && is_probably_binary(left_slice))
        || (!right_slice.is_empty() && is_probably_binary(right_slice))
    {
        return FileDiffResult {
            rel_path: rel_path.to_string(),
            is_binary: true,
            truncated: false,
            left_label,
            right_label,
            insertions: 0,
            deletions: 0,
            rows: Vec::new(),
        };
    }

    let mut truncated = false;
    let left_capped = if left_slice.len() > MAX_DIFF_BYTES {
        truncated = true;
        &left_slice[..MAX_DIFF_BYTES]
    } else {
        left_slice
    };
    let right_capped = if right_slice.len() > MAX_DIFF_BYTES {
        truncated = true;
        &right_slice[..MAX_DIFF_BYTES]
    } else {
        right_slice
    };

    let left_text = String::from_utf8_lossy(left_capped);
    let right_text = String::from_utf8_lossy(right_capped);

    let diff = TextDiff::from_lines(left_text.as_ref(), right_text.as_ref());

    let mut rows: Vec<SideBySideRow> = Vec::new();
    let mut pending_deletes: Vec<(usize, String)> = Vec::new();
    let mut pending_inserts: Vec<(usize, String)> = Vec::new();
    let mut insertions = 0usize;
    let mut deletions = 0usize;

    let flush_pending = |rows: &mut Vec<SideBySideRow>,
                         dels: &mut Vec<(usize, String)>,
                         ins: &mut Vec<(usize, String)>,
                         truncated: &mut bool| {
        let max_len = dels.len().max(ins.len());
        for i in 0..max_len {
            if rows.len() >= MAX_DIFF_ROWS {
                *truncated = true;
                break;
            }
            let left = dels.get(i);
            let right = ins.get(i);
            rows.push(SideBySideRow {
                left_line_no: left.map(|(n, _)| *n),
                left_text: left.map(|(_, s)| s.clone()),
                left_kind: left.map(|_| DiffLineKind::Delete),
                right_line_no: right.map(|(n, _)| *n),
                right_text: right.map(|(_, s)| s.clone()),
                right_kind: right.map(|_| DiffLineKind::Insert),
            });
        }
        dels.clear();
        ins.clear();
    };

    for change in diff.iter_all_changes() {
        let line = change
            .value()
            .trim_end_matches('\n')
            .trim_end_matches('\r')
            .to_string();
        match change.tag() {
            ChangeTag::Delete => {
                deletions += 1;
                let line_no = change.old_index().map(|i| i + 1).unwrap_or(0);
                pending_deletes.push((line_no, line));
            }
            ChangeTag::Insert => {
                insertions += 1;
                let line_no = change.new_index().map(|i| i + 1).unwrap_or(0);
                pending_inserts.push((line_no, line));
            }
            ChangeTag::Equal => {
                if !pending_deletes.is_empty() || !pending_inserts.is_empty() {
                    flush_pending(
                        &mut rows,
                        &mut pending_deletes,
                        &mut pending_inserts,
                        &mut truncated,
                    );
                }
                if rows.len() >= MAX_DIFF_ROWS {
                    truncated = true;
                    break;
                }
                rows.push(SideBySideRow {
                    left_line_no: change.old_index().map(|i| i + 1),
                    left_text: Some(line.clone()),
                    left_kind: Some(DiffLineKind::Equal),
                    right_line_no: change.new_index().map(|i| i + 1),
                    right_text: Some(line),
                    right_kind: Some(DiffLineKind::Equal),
                });
            }
        }
    }

    if !pending_deletes.is_empty() || !pending_inserts.is_empty() {
        flush_pending(
            &mut rows,
            &mut pending_deletes,
            &mut pending_inserts,
            &mut truncated,
        );
    }

    FileDiffResult {
        rel_path: rel_path.to_string(),
        is_binary: false,
        truncated,
        left_label,
        right_label,
        insertions,
        deletions,
        rows,
    }
}
