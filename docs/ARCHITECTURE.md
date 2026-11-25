# Agent Restore Architecture

## High-Level Overview

Agent Restore is structured as a two-layer local-first desktop architecture:

```
┌──────────────────────────────────────────────────────────────────────┐
│                   React + TypeScript Frontend (Vite)                 │
│  Dashboard · Timeline · Side-by-Side Diff · Restore Modal · Storage  │
└───────────────────────────────┬──────────────────────────────────────┘
                                │ Tauri v2 Local IPC (No Network)
┌───────────────────────────────▼──────────────────────────────────────┐
│                  Tauri Desktop Shell (src-tauri)                     │
│  IPC Commands · System Tray · Native Folder Dialog · Window Events   │
└───────────────────────────────┬──────────────────────────────────────┘
                                │
┌───────────────────────────────▼──────────────────────────────────────┐
│               Core Engine (crates/agent-restore-core)                │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌─────────────┐ │
│  │ MonitorSvc   │ │ Security &   │ │ Diff &       │ │ Retention & │ │
│  │ (notify +    │ │ Ignore Engine│ │ Restore      │ │ Garbage     │ │
│  │  debouncer)  │ │ (globset)    │ │ (similar)    │ │ Collector   │ │
│  └──────┬───────┘ └──────┬───────┘ └──────┬───────┘ └──────┬──────┘ │
│         └────────────────┴────────────────┴────────────────┘        │
│                          │                │                         │
│           ┌──────────────▼────┐   ┌───────▼──────────────┐          │
│           │  SQLite (WAL)     │   │ Content-Addressed    │          │
│           │  agent_restore.db │   │ Storage (BLAKE3+Zstd)│          │
│           └───────────────────┘   └──────────────────────┘          │
└──────────────────────────────────────────────────────────────────────┘
```

## Storage Layout

All Agent Restore data lives in the OS-standard local application data directory (or a custom path via `AGENT_RESTORE_DATA_DIR`):

```
<app_local_data>/store/
├── agent_restore.db          # SQLite database (WAL mode, synchronous=FULL)
├── agent_restore.db-wal
├── agent_restore.db-shm
├── tmp/                      # Atomic object staging (auto-cleaned on startup)
└── objects/
    └── <hh>/                 # First 2 hex chars of BLAKE3 hash
        └── <hh>/             # Next 2 hex chars of BLAKE3 hash
            └── <64-hex>.arobj
```

### `.arobj` Binary Object Format

Each object stored in `objects/` has an 8-byte header followed by the payload:

| Offset | Length | Field | Description |
| ------ | ------ | ----- | ----------- |
| `0`    | 7 bytes | Magic | ASCII `AROBJ01` |
| `7`    | 1 byte | Compression | `0` = raw, `1` = Zstd (level 3) |
| `8`    | N bytes | Payload | Raw or Zstd-compressed file bytes |

Files smaller than 128 bytes, files with already-compressed extensions (`.png`, `.zip`, `.woff2`, `.mp4`, etc.), or files where Zstd does not reduce size are stored raw (`0`). Every read verifies the magic header, decompresses if needed, and recomputes the BLAKE3 hash to verify it matches the requested object address.

## Incremental Checkpointing

1. Before scanning, the engine loads the latest completed checkpoint's `(rel_path -> size, mtime_secs, mtime_nanos, object_hash)` map.
2. `walkdir` traverses the project root with `follow_links(false)`, skipping excluded directories immediately (`skip_current_dir()`).
3. If an eligible file's `(size, mtime)` matches the previous checkpoint and its CAS object exists on disk, its `object_hash` is reused without re-reading or re-compressing.
4. New or changed files are read with a stability check (stat before and after read; retried up to 2x if mutated mid-read), hashed with BLAKE3, compressed with Zstd, and atomically placed into `objects/`.
5. All `checkpoint_files`, `cas_objects`, and `checkpoints.status = 'complete'` updates commit inside a single atomic SQLite transaction alongside a deterministic BLAKE3 `tree_hash`.

## Safe Restoration Protocol

Every restore follows a strict 6-step protocol:

1. **Preview (`preview_restore`)**: Scans live workspace, verifies BLAKE3 integrity of every target CAS object, classifies files (`restore_deleted`, `overwrite`, `delete_extra` / `keep_extra`, unchanged), and computes a deterministic `workspace_fingerprint`.
2. **Explicit Confirmation**: `execute_restore` rejects requests without `confirmed: true`.
3. **Conflict & Space Guard**: Recomputes `workspace_fingerprint` under a per-project mutex; if any affected file changed since preview, aborts with `CONCURRENT_MODIFICATION`. Verifies free disk space on both the store drive and project drive.
4. **Mandatory Pre-Restore Checkpoint**: Creates a labeled `pre_restore` checkpoint of the current workspace before modifying any file.
5. **Restore Journal**: Writes an `in_progress` row in `operations_log` referencing `preRestoreCheckpointId`.
6. **Atomic Writes**: Writes each restored file to `.agentrestore-tmp-<uuid>` in the verified parent directory, calls `sync_all()`, and atomically replaces the target (with safe backup-swap rollback on Windows).

## Crash Recovery

On startup (`AgentRestoreEngine::open`):

- Any `checkpoints` row still in `in_progress` (crash during backup) is deleted cleanly without affecting prior checkpoints.
- Orphaned files in `store/tmp/` and `.agentrestore-tmp-*` staging files in registered projects are removed.
- Any `operations_log` entry of type `restore` left in `in_progress` is flagged in `CrashRecoveryReport` with its `preRestoreCheckpointId` so the UI immediately offers one-click recovery.

## Known Limitations (Transparent by Design)

- **Discrete snapshots**: Agent Restore captures point-in-time snapshots when debounced file events settle, on scheduled intervals, or manually. Multiple rapid overwrites occurring within a single debounce window before a snapshot is captured are coalesced into one checkpoint.
- **Symlinks**: Symbolic links are intentionally never followed, archived, or overwritten for security.
- **Excluded files**: Secrets (`.env`, private keys, credentials), `.git/`, caches (`node_modules/`, `target/`), and files exceeding the configured max file size (default 50 MB) are not included in checkpoints.
- **Locked files on Windows**: Files exclusively locked by another running process (such as an open database engine) cannot be overwritten during restore until the lock is released; Agent Restore reports such failures cleanly rather than corrupting data.
