export type Language = "en" | "tr";
export type Theme = "dark" | "light";
export type TriggerType = "manual" | "auto" | "watcher" | "pre_restore";
export type ChangeKind = "added" | "modified" | "deleted" | "unchanged";
export type DiffLineKind = "equal" | "insert" | "delete";

export interface StructuredError {
  code: string;
  message: string;
  details?: Record<string, unknown>;
}

export interface AppMeta {
  version: string;
  dataDir: string;
  monitoringPaused: boolean;
  platform: string;
}

export interface ExclusionDefaults {
  security: string[];
  caches: string[];
}

export interface GlobalSettings {
  language: Language;
  theme: Theme;
  minimizeToTray: boolean;
  autoStartMonitoring: boolean;
  defaultIntervalSecs: number;
  defaultDebounceMs: number;
  defaultMaxStorageMb: number;
  defaultMaxCheckpoints: number;
  defaultRetentionDays: number;
  defaultMaxFileSizeMb: number;
  minFreeDiskMb: number;
  globalExcludes: string[];
}

export interface Project {
  id: string;
  name: string;
  rootPath: string;
  enabled: boolean;
  autoBackup: boolean;
  intervalSecs: number;
  debounceMs: number;
  maxStorageMb: number;
  maxCheckpoints: number;
  retentionDays: number;
  customExcludes: string[];
  respectGitignore: boolean;
  gitIntegration: boolean;
  maxFileSizeMb: number;
  createdAt: string;
  updatedAt: string;
  lastCheckpointAt: string | null;
  checkpointCount: number;
  storedSizeBytes: number;
  logicalSizeBytes: number;
  gitBranch: string | null;
  gitCommit: string | null;
  gitDirty: boolean;
}

export interface CreateProjectRequest {
  name?: string;
  rootPath: string;
  autoBackup?: boolean;
  intervalSecs?: number;
  debounceMs?: number;
  maxStorageMb?: number;
  maxCheckpoints?: number;
  retentionDays?: number;
  customExcludes?: string[];
  respectGitignore?: boolean;
  gitIntegration?: boolean;
  maxFileSizeMb?: number;
}

export interface UpdateProjectRequest {
  name?: string;
  enabled?: boolean;
  autoBackup?: boolean;
  intervalSecs?: number;
  debounceMs?: number;
  maxStorageMb?: number;
  maxCheckpoints?: number;
  retentionDays?: number;
  customExcludes?: string[];
  respectGitignore?: boolean;
  gitIntegration?: boolean;
  maxFileSizeMb?: number;
}

export interface Checkpoint {
  id: string;
  projectId: string;
  label: string;
  note: string;
  triggerType: TriggerType;
  status: string;
  parentCheckpointId: string | null;
  fileCount: number;
  totalSizeBytes: number;
  storedSizeBytes: number;
  addedCount: number;
  modifiedCount: number;
  deletedCount: number;
  gitBranch: string | null;
  gitCommit: string | null;
  gitDirty: boolean;
  treeHash: string;
  createdAt: string;
}

export interface CheckpointFile {
  relPath: string;
  objectHash: string;
  sizeBytes: number;
  mtimeSecs: number;
  mtimeNanos: number;
  permissions: number;
  isBinary: boolean;
  gitStatus: string;
}

export interface FileChangeEntry {
  relPath: string;
  kind: ChangeKind;
  oldSize: number | null;
  newSize: number | null;
  isBinary: boolean;
  gitStatus: string;
}

export interface CheckpointDiffSummary {
  baseCheckpointId: string | null;
  targetCheckpointId: string | null;
  comparedWithWorkspace: boolean;
  addedCount: number;
  modifiedCount: number;
  deletedCount: number;
  changes: FileChangeEntry[];
}

export interface SideBySideRow {
  leftLineNo: number | null;
  leftText: string | null;
  leftKind: DiffLineKind | null;
  rightLineNo: number | null;
  rightText: string | null;
  rightKind: DiffLineKind | null;
}

export interface FileDiffResult {
  relPath: string;
  isBinary: boolean;
  truncated: boolean;
  leftLabel: string;
  rightLabel: string;
  insertions: number;
  deletions: number;
  rows: SideBySideRow[];
}

export interface CreateCheckpointOutcome {
  checkpoint: Checkpoint | null;
  skippedUnchanged: boolean;
  warnings: string[];
}

export interface RestoreFilePlan {
  relPath: string;
  action: "restore_deleted" | "overwrite" | "delete_extra" | "keep_extra";
  currentSize: number | null;
  targetSize: number | null;
}

export interface RestorePreview {
  projectId: string;
  checkpointId: string;
  checkpointLabel: string;
  isFullRestore: boolean;
  deleteExtraFiles: boolean;
  filesToRecreate: RestoreFilePlan[];
  filesToOverwrite: RestoreFilePlan[];
  extraFilesToDelete: RestoreFilePlan[];
  extraFilesKept: RestoreFilePlan[];
  unchangedCount: number;
  totalBytesToWrite: number;
  workspaceFingerprint: string;
  integrityOk: boolean;
  corruptedObjects: string[];
}

export interface ExecuteRestoreRequest {
  projectId: string;
  checkpointId: string;
  selectedPaths: string[] | null;
  deleteExtraFiles: boolean;
  expectedFingerprint: string | null;
  confirmed: boolean;
}

export interface RestoreResult {
  preRestoreCheckpoint: Checkpoint;
  restoredCount: number;
  deletedCount: number;
  warnings: string[];
}

export interface StorageProjectBreakdown {
  projectId: string;
  projectName: string;
  checkpointCount: number;
  logicalSizeBytes: number;
  storedSizeBytes: number;
  maxStorageMb: number;
}

export interface StorageStats {
  dataDir: string;
  totalStoredBytes: number;
  totalLogicalBytes: number;
  objectCount: number;
  dedupSavingsBytes: number;
  dedupSavingsPercent: number;
  availableDiskBytes: number;
  minFreeDiskBytes: number;
  diskSpaceLow: boolean;
  projects: StorageProjectBreakdown[];
}

export interface GcReport {
  removedCheckpoints: number;
  removedObjects: number;
  reclaimedBytes: number;
}

export interface IntegrityIssue {
  checkpointId: string;
  relPath: string;
  objectHash: string;
  reason: string;
}

export interface IntegrityReport {
  checkedCheckpoints: number;
  checkedObjects: number;
  healthy: boolean;
  issues: IntegrityIssue[];
}

export interface CrashRecoveryNotice {
  opId: string;
  projectId: string;
  opType: string;
  message: string;
  recoveryCheckpointId: string | null;
  timestamp: string;
}

export interface CrashRecoveryReport {
  cleanedIncompleteCheckpoints: number;
  cleanedTempFiles: number;
  interruptedRestores: CrashRecoveryNotice[];
}

export interface WorkspaceStatus {
  projectId: string;
  hasPendingChanges: boolean;
  addedCount: number;
  modifiedCount: number;
  deletedCount: number;
  trackedFileCount: number;
  untrackedFileCount: number;
}

export interface MonitoringEvent {
  kind: "status" | "checkpoint_created" | "changes_detected" | "error";
  projectId: string | null;
  state: "monitoring" | "backing_up" | "paused" | "idle" | "error" | null;
  checkpointId: string | null;
  message: string | null;
}

export interface ToastMessage {
  id: string;
  kind: "success" | "info" | "warning" | "error";
  title: string;
  detail?: string;
}
