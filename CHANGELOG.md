# Changelog

All notable changes to **Agent Restore** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-11-25

### Added

- **Core Backup & Recovery Engine (`agent-restore-core`)**:
  - Content-addressed storage (CAS) using BLAKE3 cryptographic hashes with Zstd compression and automatic deduplication.
  - Incremental scanning reusing `(size, mtime)` metadata for unchanged files.
  - SQLite WAL metadata store with transactional checkpoint commits and crash recovery.
  - Mandatory automatic `pre_restore` safety checkpoint before every restoration.
  - Restore preview with exact file impact breakdown, BLAKE3 integrity verification, and concurrent file modification detection.
  - Atomic file writes via sibling staging files with `sync_all` and safe Windows replacement/rollback.
  - Hard-enforced path traversal protection, symlink refusal, `.git` immutability, and default secret/cache exclusions.
  - Strictly read-only Git branch/commit/untracked inspection (`GIT_OPTIONAL_LOCKS=0`).
  - Side-by-side and unified line-level code difference engine.
  - Configurable retention policies (`max_checkpoints`, `retention_days`, `max_storage_mb`), reference-counted garbage collection, and low-disk-space protection.
  - Multi-project debounced filesystem watcher with periodic interval checks and pause/resume controls.
- **Desktop Application (Tauri v2 + React + TypeScript)**:
  - Project dashboard, registered projects sidebar, interactive searchable checkpoint timeline, side-by-side code diff viewer, restore confirmation modal, storage usage monitor, and settings page.
  - Native system tray integration with quick checkpoint, pause/resume, and optional minimize-to-tray.
  - Full English (`en`) and Turkish (`tr`) localization.
  - Dark and Light minimalist themes.
- **Verification & Open-Source Infrastructure**:
  - 12 isolated integration scenario tests covering deletion, broken code, mass edits, untracked files, backup/restore crashes, large repos, low disk space, snapshot corruption, concurrent edits, path traversal, retention, and live watching.
  - Standalone release performance benchmark (`examples/benchmark.rs`).
  - Cross-platform GitHub Actions CI/CD workflows, issue/PR templates, architecture documentation, and third-party license notices.
