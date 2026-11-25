use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::models::{
    Checkpoint, CheckpointFile, CrashRecoveryNotice, GlobalSettings, Project, TriggerType,
};

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(db_path: &Path) -> CoreResult<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;
            PRAGMA synchronous = FULL;
            PRAGMA busy_timeout = 5000;
            ",
        )?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    fn init_schema(&self) -> CoreResult<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS projects (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                root_path TEXT NOT NULL UNIQUE,
                enabled INTEGER NOT NULL DEFAULT 1,
                auto_backup INTEGER NOT NULL DEFAULT 1,
                interval_secs INTEGER NOT NULL DEFAULT 180,
                debounce_ms INTEGER NOT NULL DEFAULT 2500,
                max_storage_mb INTEGER NOT NULL DEFAULT 2048,
                max_checkpoints INTEGER NOT NULL DEFAULT 200,
                retention_days INTEGER NOT NULL DEFAULT 30,
                custom_excludes_json TEXT NOT NULL DEFAULT '[]',
                respect_gitignore INTEGER NOT NULL DEFAULT 1,
                git_integration INTEGER NOT NULL DEFAULT 1,
                max_file_size_mb INTEGER NOT NULL DEFAULT 50,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                last_checkpoint_at TEXT
            );

            CREATE TABLE IF NOT EXISTS checkpoints (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                label TEXT NOT NULL,
                note TEXT NOT NULL DEFAULT '',
                trigger_type TEXT NOT NULL,
                status TEXT NOT NULL,
                parent_checkpoint_id TEXT,
                file_count INTEGER NOT NULL DEFAULT 0,
                total_size_bytes INTEGER NOT NULL DEFAULT 0,
                stored_size_bytes INTEGER NOT NULL DEFAULT 0,
                added_count INTEGER NOT NULL DEFAULT 0,
                modified_count INTEGER NOT NULL DEFAULT 0,
                deleted_count INTEGER NOT NULL DEFAULT 0,
                git_branch TEXT,
                git_commit TEXT,
                git_dirty INTEGER NOT NULL DEFAULT 0,
                tree_hash TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_checkpoints_project_created
                ON checkpoints(project_id, created_at DESC);

            CREATE TABLE IF NOT EXISTS checkpoint_files (
                checkpoint_id TEXT NOT NULL REFERENCES checkpoints(id) ON DELETE CASCADE,
                rel_path TEXT NOT NULL,
                object_hash TEXT NOT NULL,
                size_bytes INTEGER NOT NULL,
                mtime_secs INTEGER NOT NULL,
                mtime_nanos INTEGER NOT NULL,
                permissions INTEGER NOT NULL DEFAULT 0,
                is_binary INTEGER NOT NULL DEFAULT 0,
                git_status TEXT NOT NULL DEFAULT 'none',
                PRIMARY KEY (checkpoint_id, rel_path)
            );

            CREATE INDEX IF NOT EXISTS idx_checkpoint_files_hash
                ON checkpoint_files(object_hash);

            CREATE TABLE IF NOT EXISTS cas_objects (
                hash TEXT PRIMARY KEY,
                raw_size INTEGER NOT NULL,
                stored_size INTEGER NOT NULL,
                compression TEXT NOT NULL,
                ref_count INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS operations_log (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                op_type TEXT NOT NULL,
                status TEXT NOT NULL,
                details_json TEXT NOT NULL DEFAULT '{}',
                started_at TEXT NOT NULL,
                finished_at TEXT
            );
            ",
        )?;
        Ok(())
    }

    pub fn get_settings(&self) -> CoreResult<GlobalSettings> {
        let raw: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'global_settings'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        match raw {
            Some(json) => Ok(serde_json::from_str(&json).unwrap_or_default()),
            None => Ok(GlobalSettings::default()),
        }
    }

    pub fn save_settings(&self, settings: &GlobalSettings) -> CoreResult<()> {
        let json = serde_json::to_string(settings)?;
        self.conn.execute(
            "INSERT INTO settings(key, value) VALUES('global_settings', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![json],
        )?;
        Ok(())
    }

    pub fn insert_project(&self, p: &Project) -> CoreResult<()> {
        let excludes_json = serde_json::to_string(&p.custom_excludes)?;
        self.conn.execute(
            "INSERT INTO projects(
                id, name, root_path, enabled, auto_backup, interval_secs, debounce_ms,
                max_storage_mb, max_checkpoints, retention_days, custom_excludes_json,
                respect_gitignore, git_integration, max_file_size_mb, created_at, updated_at, last_checkpoint_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                p.id,
                p.name,
                p.root_path,
                p.enabled as i64,
                p.auto_backup as i64,
                p.interval_secs as i64,
                p.debounce_ms as i64,
                p.max_storage_mb as i64,
                p.max_checkpoints as i64,
                p.retention_days as i64,
                excludes_json,
                p.respect_gitignore as i64,
                p.git_integration as i64,
                p.max_file_size_mb as i64,
                p.created_at,
                p.updated_at,
                p.last_checkpoint_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_project(&self, p: &Project) -> CoreResult<()> {
        let excludes_json = serde_json::to_string(&p.custom_excludes)?;
        self.conn.execute(
            "UPDATE projects SET
                name = ?2, enabled = ?3, auto_backup = ?4, interval_secs = ?5, debounce_ms = ?6,
                max_storage_mb = ?7, max_checkpoints = ?8, retention_days = ?9,
                custom_excludes_json = ?10, respect_gitignore = ?11, git_integration = ?12,
                max_file_size_mb = ?13, updated_at = ?14, last_checkpoint_at = ?15
             WHERE id = ?1",
            params![
                p.id,
                p.name,
                p.enabled as i64,
                p.auto_backup as i64,
                p.interval_secs as i64,
                p.debounce_ms as i64,
                p.max_storage_mb as i64,
                p.max_checkpoints as i64,
                p.retention_days as i64,
                excludes_json,
                p.respect_gitignore as i64,
                p.git_integration as i64,
                p.max_file_size_mb as i64,
                p.updated_at,
                p.last_checkpoint_at,
            ],
        )?;
        Ok(())
    }

    pub fn delete_project(&self, project_id: &str) -> CoreResult<()> {
        let affected = self
            .conn
            .execute("DELETE FROM projects WHERE id = ?1", params![project_id])?;
        if affected == 0 {
            return Err(CoreError::ProjectNotFound(project_id.to_string()));
        }
        Ok(())
    }

    pub fn get_project(&self, project_id: &str) -> CoreResult<Project> {
        let mut stmt = self.conn.prepare(
            "SELECT
                p.id, p.name, p.root_path, p.enabled, p.auto_backup, p.interval_secs, p.debounce_ms,
                p.max_storage_mb, p.max_checkpoints, p.retention_days, p.custom_excludes_json,
                p.respect_gitignore, p.git_integration, p.max_file_size_mb, p.created_at, p.updated_at,
                p.last_checkpoint_at,
                (SELECT COUNT(*) FROM checkpoints c WHERE c.project_id = p.id AND c.status = 'complete'),
                (SELECT COALESCE(SUM(c.total_size_bytes), 0) FROM checkpoints c WHERE c.project_id = p.id AND c.status = 'complete')
             FROM projects p WHERE p.id = ?1",
        )?;

        let mut project = stmt
            .query_row(params![project_id], Self::map_project_row)
            .optional()?
            .ok_or_else(|| CoreError::ProjectNotFound(project_id.to_string()))?;

        project.stored_size_bytes = self.project_unique_stored_bytes(&project.id)?;
        Ok(project)
    }

    pub fn find_project_by_root(&self, root_path: &str) -> CoreResult<Option<String>> {
        let id = self
            .conn
            .query_row(
                "SELECT id FROM projects WHERE root_path = ?1",
                params![root_path],
                |row| row.get(0),
            )
            .optional()?;
        Ok(id)
    }

    pub fn list_projects(&self) -> CoreResult<Vec<Project>> {
        let mut stmt = self.conn.prepare(
            "SELECT
                p.id, p.name, p.root_path, p.enabled, p.auto_backup, p.interval_secs, p.debounce_ms,
                p.max_storage_mb, p.max_checkpoints, p.retention_days, p.custom_excludes_json,
                p.respect_gitignore, p.git_integration, p.max_file_size_mb, p.created_at, p.updated_at,
                p.last_checkpoint_at,
                (SELECT COUNT(*) FROM checkpoints c WHERE c.project_id = p.id AND c.status = 'complete'),
                (SELECT COALESCE(SUM(c.total_size_bytes), 0) FROM checkpoints c WHERE c.project_id = p.id AND c.status = 'complete')
             FROM projects p ORDER BY p.created_at ASC",
        )?;
        let rows = stmt.query_map([], Self::map_project_row)?;
        let mut projects = Vec::new();
        for row in rows {
            let mut p = row?;
            p.stored_size_bytes = self.project_unique_stored_bytes(&p.id)?;
            projects.push(p);
        }
        Ok(projects)
    }

    pub fn project_unique_stored_bytes(&self, project_id: &str) -> CoreResult<u64> {
        let bytes: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(o.stored_size), 0)
             FROM cas_objects o
             WHERE o.hash IN (
                 SELECT DISTINCT cf.object_hash
                 FROM checkpoint_files cf
                 INNER JOIN checkpoints c ON c.id = cf.checkpoint_id
                 WHERE c.project_id = ?1 AND c.status = 'complete'
             )",
            params![project_id],
            |row| row.get(0),
        )?;
        Ok(bytes.max(0) as u64)
    }

    fn map_project_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
        let excludes_raw: String = row.get(10)?;
        let custom_excludes: Vec<String> = serde_json::from_str(&excludes_raw).unwrap_or_default();
        Ok(Project {
            id: row.get(0)?,
            name: row.get(1)?,
            root_path: row.get(2)?,
            enabled: row.get::<_, i64>(3)? != 0,
            auto_backup: row.get::<_, i64>(4)? != 0,
            interval_secs: row.get::<_, i64>(5)?.max(5) as u64,
            debounce_ms: row.get::<_, i64>(6)?.max(250) as u64,
            max_storage_mb: row.get::<_, i64>(7)?.max(16) as u64,
            max_checkpoints: row.get::<_, i64>(8)?.max(1) as u32,
            retention_days: row.get::<_, i64>(9)?.max(1) as u32,
            custom_excludes,
            respect_gitignore: row.get::<_, i64>(11)? != 0,
            git_integration: row.get::<_, i64>(12)? != 0,
            max_file_size_mb: row.get::<_, i64>(13)?.max(1) as u64,
            created_at: row.get(14)?,
            updated_at: row.get(15)?,
            last_checkpoint_at: row.get(16)?,
            checkpoint_count: row.get::<_, i64>(17)?.max(0) as u64,
            logical_size_bytes: row.get::<_, i64>(18)?.max(0) as u64,
            stored_size_bytes: 0,
            git_branch: None,
            git_commit: None,
            git_dirty: false,
        })
    }

    pub fn begin_checkpoint_record(
        &self,
        checkpoint_id: &str,
        project_id: &str,
        label: &str,
        note: &str,
        trigger_type: TriggerType,
        parent_id: Option<&str>,
        created_at: &str,
    ) -> CoreResult<()> {
        self.conn.execute(
            "INSERT INTO checkpoints(
                id, project_id, label, note, trigger_type, status, parent_checkpoint_id, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, 'in_progress', ?6, ?7)",
            params![
                checkpoint_id,
                project_id,
                label,
                note,
                trigger_type.as_str(),
                parent_id,
                created_at,
            ],
        )?;
        Ok(())
    }

    pub fn abort_in_progress_checkpoint(&self, checkpoint_id: &str) -> CoreResult<()> {
        self.conn.execute(
            "DELETE FROM checkpoints WHERE id = ?1 AND status = 'in_progress'",
            params![checkpoint_id],
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn finalize_checkpoint_transaction(
        &mut self,
        checkpoint: &Checkpoint,
        files: &[CheckpointFile],
        new_objects: &[(String, u64, u64, String)],
    ) -> CoreResult<()> {
        let tx = self.conn.transaction()?;

        {
            let mut obj_stmt = tx.prepare(
                "INSERT INTO cas_objects(hash, raw_size, stored_size, compression, ref_count, created_at)
                 VALUES (?1, ?2, ?3, ?4, 0, ?5)
                 ON CONFLICT(hash) DO NOTHING",
            )?;
            let now = Utc::now().to_rfc3339();
            for (hash, raw_size, stored_size, comp) in new_objects {
                obj_stmt.execute(params![
                    hash,
                    *raw_size as i64,
                    *stored_size as i64,
                    comp,
                    now
                ])?;
            }
        }

        {
            let mut file_stmt = tx.prepare(
                "INSERT INTO checkpoint_files(
                    checkpoint_id, rel_path, object_hash, size_bytes, mtime_secs, mtime_nanos,
                    permissions, is_binary, git_status
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )?;
            for f in files {
                file_stmt.execute(params![
                    checkpoint.id,
                    f.rel_path,
                    f.object_hash,
                    f.size_bytes as i64,
                    f.mtime_secs,
                    f.mtime_nanos as i64,
                    f.permissions as i64,
                    f.is_binary as i64,
                    f.git_status,
                ])?;
            }
        }

        tx.execute(
            "UPDATE checkpoints SET
                status = 'complete',
                file_count = ?2,
                total_size_bytes = ?3,
                stored_size_bytes = ?4,
                added_count = ?5,
                modified_count = ?6,
                deleted_count = ?7,
                git_branch = ?8,
                git_commit = ?9,
                git_dirty = ?10,
                tree_hash = ?11
             WHERE id = ?1",
            params![
                checkpoint.id,
                checkpoint.file_count as i64,
                checkpoint.total_size_bytes as i64,
                checkpoint.stored_size_bytes as i64,
                checkpoint.added_count as i64,
                checkpoint.modified_count as i64,
                checkpoint.deleted_count as i64,
                checkpoint.git_branch,
                checkpoint.git_commit,
                checkpoint.git_dirty as i64,
                checkpoint.tree_hash,
            ],
        )?;

        tx.execute(
            "UPDATE projects SET last_checkpoint_at = ?2, updated_at = ?2 WHERE id = ?1",
            params![checkpoint.project_id, checkpoint.created_at],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn get_latest_checkpoint(&self, project_id: &str) -> CoreResult<Option<Checkpoint>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project_id, label, note, trigger_type, status, parent_checkpoint_id,
                    file_count, total_size_bytes, stored_size_bytes, added_count, modified_count,
                    deleted_count, git_branch, git_commit, git_dirty, tree_hash, created_at
             FROM checkpoints
             WHERE project_id = ?1 AND status = 'complete'
             ORDER BY created_at DESC, rowid DESC
             LIMIT 1",
        )?;
        let cp = stmt
            .query_row(params![project_id], Self::map_checkpoint_row)
            .optional()?;
        Ok(cp)
    }

    pub fn get_checkpoint(&self, checkpoint_id: &str) -> CoreResult<Checkpoint> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project_id, label, note, trigger_type, status, parent_checkpoint_id,
                    file_count, total_size_bytes, stored_size_bytes, added_count, modified_count,
                    deleted_count, git_branch, git_commit, git_dirty, tree_hash, created_at
             FROM checkpoints WHERE id = ?1",
        )?;
        stmt.query_row(params![checkpoint_id], Self::map_checkpoint_row)
            .optional()?
            .ok_or_else(|| CoreError::CheckpointNotFound(checkpoint_id.to_string()))
    }

    pub fn list_checkpoints(
        &self,
        project_id: &str,
        query: Option<&str>,
        trigger_filter: Option<&str>,
        limit: Option<u32>,
    ) -> CoreResult<Vec<Checkpoint>> {
        let limit_val = limit.unwrap_or(500) as i64;
        let q = query.map(|s| s.trim()).filter(|s| !s.is_empty());
        let tf = trigger_filter
            .map(|s| s.trim())
            .filter(|s| !s.is_empty() && *s != "all");

        let mut sql = String::from(
            "SELECT c.id, c.project_id, c.label, c.note, c.trigger_type, c.status, c.parent_checkpoint_id,
                    c.file_count, c.total_size_bytes, c.stored_size_bytes, c.added_count, c.modified_count,
                    c.deleted_count, c.git_branch, c.git_commit, c.git_dirty, c.tree_hash, c.created_at
             FROM checkpoints c
             WHERE c.project_id = ?1 AND c.status = 'complete'",
        );

        let mut bind_params: Vec<rusqlite::types::Value> = vec![project_id.to_string().into()];

        if let Some(t) = tf {
            bind_params.push(t.to_string().into());
            sql.push_str(&format!(" AND c.trigger_type = ?{}", bind_params.len()));
        }

        if let Some(search) = q {
            let pattern = format!("%{}%", search.to_lowercase());
            bind_params.push(pattern.into());
            let idx = bind_params.len();
            sql.push_str(&format!(
                " AND (LOWER(c.label) LIKE ?{idx} OR LOWER(c.note) LIKE ?{idx} OR LOWER(COALESCE(c.git_branch,'')) LIKE ?{idx}
                  OR EXISTS (SELECT 1 FROM checkpoint_files cf WHERE cf.checkpoint_id = c.id AND LOWER(cf.rel_path) LIKE ?{idx}))"
            ));
        }

        bind_params.push(limit_val.into());
        sql.push_str(&format!(
            " ORDER BY c.created_at DESC, c.rowid DESC LIMIT ?{}",
            bind_params.len()
        ));

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(bind_params), Self::map_checkpoint_row)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn rename_checkpoint(
        &self,
        checkpoint_id: &str,
        label: &str,
        note: Option<&str>,
    ) -> CoreResult<Checkpoint> {
        let updated = if let Some(n) = note {
            self.conn.execute(
                "UPDATE checkpoints SET label = ?2, note = ?3 WHERE id = ?1",
                params![checkpoint_id, label.trim(), n.trim()],
            )?
        } else {
            self.conn.execute(
                "UPDATE checkpoints SET label = ?2 WHERE id = ?1",
                params![checkpoint_id, label.trim()],
            )?
        };
        if updated == 0 {
            return Err(CoreError::CheckpointNotFound(checkpoint_id.to_string()));
        }
        self.get_checkpoint(checkpoint_id)
    }

    pub fn delete_checkpoint(&self, checkpoint_id: &str) -> CoreResult<()> {
        let affected = self
            .conn
            .execute("DELETE FROM checkpoints WHERE id = ?1", params![checkpoint_id])?;
        if affected == 0 {
            return Err(CoreError::CheckpointNotFound(checkpoint_id.to_string()));
        }
        Ok(())
    }

    pub fn get_checkpoint_files(&self, checkpoint_id: &str) -> CoreResult<Vec<CheckpointFile>> {
        let mut stmt = self.conn.prepare(
            "SELECT rel_path, object_hash, size_bytes, mtime_secs, mtime_nanos, permissions, is_binary, git_status
             FROM checkpoint_files
             WHERE checkpoint_id = ?1
             ORDER BY rel_path ASC",
        )?;
        let rows = stmt.query_map(params![checkpoint_id], |row| {
            Ok(CheckpointFile {
                rel_path: row.get(0)?,
                object_hash: row.get(1)?,
                size_bytes: row.get::<_, i64>(2)?.max(0) as u64,
                mtime_secs: row.get(3)?,
                mtime_nanos: row.get::<_, i64>(4)?.max(0) as u32,
                permissions: row.get::<_, i64>(5)?.max(0) as u32,
                is_binary: row.get::<_, i64>(6)? != 0,
                git_status: row.get(7)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn get_checkpoint_file_map(
        &self,
        checkpoint_id: &str,
    ) -> CoreResult<HashMap<String, CheckpointFile>> {
        let list = self.get_checkpoint_files(checkpoint_id)?;
        Ok(list
            .into_iter()
            .map(|f| (f.rel_path.clone(), f))
            .collect())
    }

    fn map_checkpoint_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Checkpoint> {
        let trigger_raw: String = row.get(4)?;
        Ok(Checkpoint {
            id: row.get(0)?,
            project_id: row.get(1)?,
            label: row.get(2)?,
            note: row.get(3)?,
            trigger_type: TriggerType::parse(&trigger_raw),
            status: row.get(5)?,
            parent_checkpoint_id: row.get(6)?,
            file_count: row.get::<_, i64>(7)?.max(0) as u64,
            total_size_bytes: row.get::<_, i64>(8)?.max(0) as u64,
            stored_size_bytes: row.get::<_, i64>(9)?.max(0) as u64,
            added_count: row.get::<_, i64>(10)?.max(0) as u64,
            modified_count: row.get::<_, i64>(11)?.max(0) as u64,
            deleted_count: row.get::<_, i64>(12)?.max(0) as u64,
            git_branch: row.get(13)?,
            git_commit: row.get(14)?,
            git_dirty: row.get::<_, i64>(15)? != 0,
            tree_hash: row.get(16)?,
            created_at: row.get(17)?,
        })
    }

    pub fn start_operation(
        &self,
        project_id: &str,
        op_type: &str,
        details: &serde_json::Value,
    ) -> CoreResult<String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO operations_log(id, project_id, op_type, status, details_json, started_at)
             VALUES (?1, ?2, ?3, 'in_progress', ?4, ?5)",
            params![id, project_id, op_type, details.to_string(), now],
        )?;
        Ok(id)
    }

    pub fn finish_operation(
        &self,
        op_id: &str,
        status: &str,
        details: Option<&serde_json::Value>,
    ) -> CoreResult<()> {
        let now = Utc::now().to_rfc3339();
        if let Some(d) = details {
            self.conn.execute(
                "UPDATE operations_log SET status = ?2, details_json = ?3, finished_at = ?4 WHERE id = ?1",
                params![op_id, status, d.to_string(), now],
            )?;
        } else {
            self.conn.execute(
                "UPDATE operations_log SET status = ?2, finished_at = ?3 WHERE id = ?1",
                params![op_id, status, now],
            )?;
        }
        Ok(())
    }

    /// Detects incomplete checkpoints or interrupted restores left by an unexpected process termination.
    pub fn recover_incomplete_state(&mut self) -> CoreResult<(u64, Vec<CrashRecoveryNotice>)> {
        let tx = self.conn.transaction()?;
        let cleaned_checkpoints = tx.execute(
            "DELETE FROM checkpoints WHERE status = 'in_progress'",
            [],
        )? as u64;

        let mut notices = Vec::new();
        {
            let mut stmt = tx.prepare(
                "SELECT id, project_id, op_type, details_json, started_at
                 FROM operations_log
                 WHERE status = 'in_progress'",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })?;
            for r in rows {
                let (id, project_id, op_type, details_json, started_at) = r?;
                if op_type == "restore" {
                    let parsed: serde_json::Value =
                        serde_json::from_str(&details_json).unwrap_or_default();
                    let recovery_cp = parsed
                        .get("preRestoreCheckpointId")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    notices.push(CrashRecoveryNotice {
                        op_id: id,
                        project_id,
                        op_type,
                        message: "An interrupted restoration was detected from a previous session. You can safely roll back using the automatic pre-restore checkpoint."
                            .to_string(),
                        recovery_checkpoint_id: recovery_cp,
                        timestamp: started_at,
                    });
                }
            }
        }

        let now = Utc::now().to_rfc3339();
        tx.execute(
            "UPDATE operations_log SET status = 'interrupted', finished_at = ?1 WHERE status = 'in_progress'",
            params![now],
        )?;

        tx.commit()?;
        Ok((cleaned_checkpoints, notices))
    }

    /// Recomputes reference counts and returns hashes of unreferenced CAS objects ready for deletion.
    pub fn collect_unreferenced_objects(&mut self) -> CoreResult<Vec<String>> {
        let tx = self.conn.transaction()?;
        tx.execute("UPDATE cas_objects SET ref_count = 0", [])?;
        tx.execute(
            "UPDATE cas_objects
             SET ref_count = (
                 SELECT COUNT(*) FROM checkpoint_files cf
                 INNER JOIN checkpoints c ON c.id = cf.checkpoint_id
                 WHERE cf.object_hash = cas_objects.hash AND c.status = 'complete'
             )",
            [],
        )?;

        let mut orphan_hashes = Vec::new();
        {
            let mut stmt = tx.prepare("SELECT hash FROM cas_objects WHERE ref_count = 0")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            for h in rows {
                orphan_hashes.push(h?);
            }
        }

        tx.execute("DELETE FROM cas_objects WHERE ref_count = 0", [])?;
        tx.commit()?;
        Ok(orphan_hashes)
    }

    pub fn cas_totals(&self) -> CoreResult<(u64, u64, u64)> {
        let (count, stored, raw): (i64, i64, i64) = self.conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(stored_size), 0), COALESCE(SUM(raw_size), 0) FROM cas_objects",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        Ok((
            count.max(0) as u64,
            stored.max(0) as u64,
            raw.max(0) as u64,
        ))
    }

    pub fn total_logical_bytes(&self) -> CoreResult<u64> {
        let total: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(total_size_bytes), 0) FROM checkpoints WHERE status = 'complete'",
            [],
            |row| row.get(0),
        )?;
        Ok(total.max(0) as u64)
    }

    pub fn all_referenced_hashes(&self) -> CoreResult<HashSet<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT cf.object_hash
             FROM checkpoint_files cf
             INNER JOIN checkpoints c ON c.id = cf.checkpoint_id
             WHERE c.status = 'complete'",
        )?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut set = HashSet::new();
        for h in rows {
            set.insert(h?);
        }
        Ok(set)
    }
}
