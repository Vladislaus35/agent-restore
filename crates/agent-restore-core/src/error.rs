use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("Project not found: {0}")]
    ProjectNotFound(String),

    #[error("Project already registered: {0}")]
    ProjectAlreadyExists(String),

    #[error("Checkpoint not found: {0}")]
    CheckpointNotFound(String),

    #[error("File not found in checkpoint: {0}")]
    FileNotInCheckpoint(String),

    #[error("Invalid or unauthorized path: {0}")]
    UnauthorizedPath(String),

    #[error("Path traversal attempt blocked: {0}")]
    PathTraversal(String),

    #[error("Protected .git directory access blocked: {0}")]
    ProtectedGitPath(String),

    #[error("Unsafe symbolic link blocked: {0}")]
    UnsafeSymlink(String),

    #[error("Insufficient disk space on {path}: available {available_bytes} bytes, required {required_bytes} bytes")]
    InsufficientDiskSpace {
        path: String,
        available_bytes: u64,
        required_bytes: u64,
    },

    #[error("Corrupted snapshot detected for object {hash}: {reason}")]
    CorruptedSnapshot { hash: String, reason: String },

    #[error("Concurrent file modification detected on {paths:?}. Refresh preview before restoring.")]
    ConcurrentModification { paths: Vec<String> },

    #[error("Restore confirmation required for destructive operation")]
    ConfirmationRequired,

    #[error("Invalid glob exclusion pattern '{pattern}': {reason}")]
    InvalidPattern { pattern: String, reason: String },

    #[error("Project directory does not exist or is not a directory: {0}")]
    InvalidProjectDirectory(String),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Internal lock poisoned")]
    LockPoisoned,
}

#[derive(Debug, Clone, Serialize)]
pub struct StructuredError {
    pub code: &'static str,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl CoreError {
    pub fn code(&self) -> &'static str {
        match self {
            CoreError::ProjectNotFound(_) => "PROJECT_NOT_FOUND",
            CoreError::ProjectAlreadyExists(_) => "PROJECT_ALREADY_EXISTS",
            CoreError::CheckpointNotFound(_) => "CHECKPOINT_NOT_FOUND",
            CoreError::FileNotInCheckpoint(_) => "FILE_NOT_IN_CHECKPOINT",
            CoreError::UnauthorizedPath(_) => "UNAUTHORIZED_PATH",
            CoreError::PathTraversal(_) => "PATH_TRAVERSAL",
            CoreError::ProtectedGitPath(_) => "PROTECTED_GIT_PATH",
            CoreError::UnsafeSymlink(_) => "UNSAFE_SYMLINK",
            CoreError::InsufficientDiskSpace { .. } => "INSUFFICIENT_DISK_SPACE",
            CoreError::CorruptedSnapshot { .. } => "CORRUPTED_SNAPSHOT",
            CoreError::ConcurrentModification { .. } => "CONCURRENT_MODIFICATION",
            CoreError::ConfirmationRequired => "CONFIRMATION_REQUIRED",
            CoreError::InvalidPattern { .. } => "INVALID_PATTERN",
            CoreError::InvalidProjectDirectory(_) => "INVALID_PROJECT_DIRECTORY",
            CoreError::Database(_) => "DATABASE_ERROR",
            CoreError::Io(_) => "IO_ERROR",
            CoreError::Serde(_) => "SERIALIZATION_ERROR",
            CoreError::LockPoisoned => "LOCK_POISONED",
        }
    }

    pub fn to_structured(&self) -> StructuredError {
        let details = match self {
            CoreError::InsufficientDiskSpace {
                path,
                available_bytes,
                required_bytes,
            } => Some(serde_json::json!({
                "path": path,
                "availableBytes": available_bytes,
                "requiredBytes": required_bytes,
            })),
            CoreError::CorruptedSnapshot { hash, reason } => Some(serde_json::json!({
                "hash": hash,
                "reason": reason,
            })),
            CoreError::ConcurrentModification { paths } => Some(serde_json::json!({
                "paths": paths,
            })),
            _ => None,
        };
        StructuredError {
            code: self.code(),
            message: self.to_string(),
            details,
        }
    }
}

pub type CoreResult<T> = Result<T, CoreError>;
