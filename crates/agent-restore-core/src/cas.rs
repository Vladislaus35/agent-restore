use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};

/// 8-byte binary header for CAS objects:
/// Magic "AROBJ01" (7 bytes) + Compression flag (1 byte: 0 = raw, 1 = zstd)
const MAGIC: &[u8; 7] = b"AROBJ01";
const COMP_RAW: u8 = 0;
const COMP_ZSTD: u8 = 1;
const ZSTD_LEVEL: i32 = 3;

#[derive(Debug, Clone)]
pub struct StoredObjectMeta {
    pub hash: String,
    pub raw_size: u64,
    pub stored_size: u64,
    pub compression: &'static str,
    pub newly_written: bool,
}

#[derive(Debug, Clone)]
pub struct CasStore {
    objects_dir: PathBuf,
    tmp_dir: PathBuf,
}

impl CasStore {
    pub fn open(data_dir: &Path) -> CoreResult<Self> {
        let objects_dir = data_dir.join("objects");
        let tmp_dir = data_dir.join("tmp");
        fs::create_dir_all(&objects_dir)?;
        fs::create_dir_all(&tmp_dir)?;
        Ok(Self {
            objects_dir,
            tmp_dir,
        })
    }

    pub fn objects_dir(&self) -> &Path {
        &self.objects_dir
    }

    pub fn tmp_dir(&self) -> &Path {
        &self.tmp_dir
    }

    pub fn object_path(&self, hash: &str) -> CoreResult<PathBuf> {
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(CoreError::CorruptedSnapshot {
                hash: hash.to_string(),
                reason: "Invalid BLAKE3 object hash format".to_string(),
            });
        }
        Ok(self
            .objects_dir
            .join(&hash[0..2])
            .join(&hash[2..4])
            .join(format!("{}.arobj", hash)))
    }

    pub fn has_object(&self, hash: &str) -> bool {
        self.object_path(hash).map(|p| p.is_file()).unwrap_or(false)
    }

    /// Checks available disk space on the given directory against `required_bytes + min_free_bytes`.
    pub fn check_disk_space(
        target_dir: &Path,
        required_bytes: u64,
        min_free_bytes: u64,
    ) -> CoreResult<u64> {
        let available = fs4::available_space(target_dir)?;
        let needed = required_bytes.saturating_add(min_free_bytes);
        if available < needed {
            return Err(CoreError::InsufficientDiskSpace {
                path: target_dir.display().to_string(),
                available_bytes: available,
                required_bytes: needed,
            });
        }
        Ok(available)
    }

    /// Stores raw bytes in content-addressed storage with Zstd compression and atomic rename.
    pub fn put_bytes(
        &self,
        data: &[u8],
        rel_path_hint: &str,
        min_free_bytes: u64,
    ) -> CoreResult<StoredObjectMeta> {
        let hash = blake3::hash(data).to_hex().to_string();
        let obj_path = self.object_path(&hash)?;

        if obj_path.is_file() {
            let stored_size = fs::metadata(&obj_path).map(|m| m.len()).unwrap_or(0);
            return Ok(StoredObjectMeta {
                hash,
                raw_size: data.len() as u64,
                stored_size,
                compression: "dedup",
                newly_written: false,
            });
        }

        // Decide whether to compress: skip tiny files or already-compressed formats
        let (comp_flag, comp_label, payload): (u8, &'static str, Vec<u8>) =
            if data.len() >= 128 && !is_already_compressed_ext(rel_path_hint) {
                match zstd::bulk::compress(data, ZSTD_LEVEL) {
                    Ok(compressed) if compressed.len() + 8 < data.len() => {
                        (COMP_ZSTD, "zstd", compressed)
                    }
                    _ => (COMP_RAW, "raw", data.to_vec()),
                }
            } else {
                (COMP_RAW, "raw", data.to_vec())
            };

        let total_write_size = (MAGIC.len() + 1 + payload.len()) as u64;
        if total_write_size >= 256 * 1024 {
            Self::check_disk_space(&self.objects_dir, total_write_size, min_free_bytes)?;
        }

        if let Some(parent) = obj_path.parent() {
            if !parent.is_dir() {
                fs::create_dir_all(parent)?;
            }
        }

        let tmp_path = self
            .tmp_dir
            .join(format!("obj-{}.tmp", Uuid::new_v4().simple()));
        {
            let mut file = File::create(&tmp_path)?;
            file.write_all(MAGIC)?;
            file.write_all(&[comp_flag])?;
            file.write_all(&payload)?;
            file.flush()?;
        }

        // Atomic move into CAS; if another thread/process already placed the same hash, discard tmp safely.
        if let Err(err) = fs::rename(&tmp_path, &obj_path) {
            let _ = fs::remove_file(&tmp_path);
            if !obj_path.is_file() {
                return Err(CoreError::Io(err));
            }
        }

        Ok(StoredObjectMeta {
            hash,
            raw_size: data.len() as u64,
            stored_size: total_write_size,
            compression: comp_label,
            newly_written: true,
        })
    }

    /// Reads an object from CAS and cryptographically verifies its BLAKE3 hash before returning bytes.
    pub fn get_verified_bytes(&self, expected_hash: &str) -> CoreResult<Vec<u8>> {
        let obj_path = self.object_path(expected_hash)?;
        let raw_disk = fs::read(&obj_path).map_err(|e| CoreError::CorruptedSnapshot {
            hash: expected_hash.to_string(),
            reason: format!("Object file unreadable or missing: {}", e),
        })?;

        if raw_disk.len() < MAGIC.len() + 1 || &raw_disk[..MAGIC.len()] != MAGIC {
            return Err(CoreError::CorruptedSnapshot {
                hash: expected_hash.to_string(),
                reason: "Invalid object magic header".to_string(),
            });
        }

        let comp_flag = raw_disk[MAGIC.len()];
        let payload = &raw_disk[MAGIC.len() + 1..];

        let decoded = match comp_flag {
            COMP_RAW => payload.to_vec(),
            COMP_ZSTD => {
                zstd::stream::decode_all(payload).map_err(|e| CoreError::CorruptedSnapshot {
                    hash: expected_hash.to_string(),
                    reason: format!("Zstd decompression failed: {}", e),
                })?
            }
            other => {
                return Err(CoreError::CorruptedSnapshot {
                    hash: expected_hash.to_string(),
                    reason: format!("Unknown compression flag: {}", other),
                })
            }
        };

        let actual_hash = blake3::hash(&decoded).to_hex().to_string();
        if actual_hash != expected_hash {
            return Err(CoreError::CorruptedSnapshot {
                hash: expected_hash.to_string(),
                reason: format!("BLAKE3 hash mismatch (actual: {})", actual_hash),
            });
        }

        Ok(decoded)
    }

    /// Verifies object integrity without keeping decompressed data.
    pub fn verify_object(&self, expected_hash: &str) -> CoreResult<u64> {
        let bytes = self.get_verified_bytes(expected_hash)?;
        Ok(bytes.len() as u64)
    }

    /// Deletes an unreferenced object during garbage collection.
    pub fn delete_object(&self, hash: &str) -> CoreResult<u64> {
        let obj_path = self.object_path(hash)?;
        if obj_path.is_file() {
            let size = fs::metadata(&obj_path).map(|m| m.len()).unwrap_or(0);
            fs::remove_file(&obj_path)?;
            return Ok(size);
        }
        Ok(0)
    }

    /// Cleans orphaned temporary object files created before an abrupt crash.
    pub fn cleanup_tmp_dir(&self) -> u64 {
        let mut cleaned = 0u64;
        if let Ok(entries) = fs::read_dir(&self.tmp_dir) {
            for entry in entries.flatten() {
                if entry.path().is_file() && fs::remove_file(entry.path()).is_ok() {
                    cleaned += 1;
                }
            }
        }
        cleaned
    }
}

fn is_already_compressed_ext(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    for ext in [
        ".png", ".jpg", ".jpeg", ".webp", ".gif", ".avif", ".zip", ".gz", ".tgz", ".bz2", ".xz",
        ".7z", ".rar", ".zst", ".woff", ".woff2", ".mp4", ".webm", ".mkv", ".mp3", ".ogg", ".flac",
        ".pdf", ".jar", ".whl",
    ] {
        if lower.ends_with(ext) {
            return true;
        }
    }
    false
}
