import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type {
  AppMeta,
  Checkpoint,
  CheckpointDiffSummary,
  CheckpointFile,
  CrashRecoveryReport,
  CreateCheckpointOutcome,
  CreateProjectRequest,
  ExclusionDefaults,
  ExecuteRestoreRequest,
  FileDiffResult,
  GcReport,
  GlobalSettings,
  IntegrityReport,
  Project,
  RestorePreview,
  RestoreResult,
  StorageStats,
  StructuredError,
  UpdateProjectRequest,
  WorkspaceStatus,
} from "./types";

export function parseBackendError(err: unknown): StructuredError {
  if (typeof err === "object" && err !== null && "code" in err && "message" in err) {
    const e = err as StructuredError;
    return {
      code: String(e.code),
      message: String(e.message),
      details: e.details,
    };
  }
  if (typeof err === "string") {
    try {
      const parsed = JSON.parse(err) as StructuredError;
      if (parsed.code && parsed.message) return parsed;
    } catch {
      // plain string
    }
    return { code: "UNKNOWN_ERROR", message: err };
  }
  if (err instanceof Error) {
    return { code: "UNKNOWN_ERROR", message: err.message };
  }
  return { code: "UNKNOWN_ERROR", message: "An unexpected error occurred." };
}

export const api = {
  getAppMeta: () => invoke<AppMeta>("get_app_meta"),
  getDefaultExclusions: () => invoke<ExclusionDefaults>("get_default_exclusions"),
  getCrashRecoveryReport: () => invoke<CrashRecoveryReport>("get_crash_recovery_report"),
  dismissCrashRecoveryNotices: () => invoke<void>("dismiss_crash_recovery_notices"),

  getSettings: () => invoke<GlobalSettings>("get_settings"),
  saveSettings: (settings: GlobalSettings) => invoke<GlobalSettings>("save_settings", { settings }),
  setMonitoringPaused: (paused: boolean) => invoke<boolean>("set_monitoring_paused", { paused }),

  pickProjectDirectory: async (): Promise<string | null> => {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: "Select Project Directory",
    });
    if (typeof selected === "string" && selected.trim().length > 0) {
      return selected;
    }
    return null;
  },

  listProjects: () => invoke<Project[]>("list_projects"),
  getProject: (projectId: string) => invoke<Project>("get_project", { projectId }),
  addProject: (request: CreateProjectRequest) => invoke<Project>("add_project", { request }),
  updateProject: (projectId: string, request: UpdateProjectRequest) =>
    invoke<Project>("update_project", { projectId, request }),
  removeProject: (projectId: string) => invoke<GcReport>("remove_project", { projectId }),

  createCheckpoint: (
    projectId: string,
    label?: string,
    note?: string,
    forceIfUnchanged = true,
  ) =>
    invoke<CreateCheckpointOutcome>("create_checkpoint", {
      projectId,
      label: label || null,
      note: note || null,
      forceIfUnchanged,
    }),

  listCheckpoints: (
    projectId: string,
    query?: string,
    triggerFilter?: string,
    limit?: number,
  ) =>
    invoke<Checkpoint[]>("list_checkpoints", {
      projectId,
      query: query || null,
      triggerFilter: triggerFilter || null,
      limit: limit ?? 300,
    }),

  renameCheckpoint: (checkpointId: string, label: string, note?: string) =>
    invoke<Checkpoint>("rename_checkpoint", {
      checkpointId,
      label,
      note: note ?? null,
    }),

  deleteCheckpoint: (checkpointId: string) =>
    invoke<GcReport>("delete_checkpoint", { checkpointId }),

  getCheckpointFiles: (checkpointId: string) =>
    invoke<CheckpointFile[]>("get_checkpoint_files", { checkpointId }),

  diffCheckpoints: (targetCheckpointId: string, baseCheckpointId?: string) =>
    invoke<CheckpointDiffSummary>("diff_checkpoints", {
      baseCheckpointId: baseCheckpointId ?? null,
      targetCheckpointId,
    }),

  diffCheckpointWithWorkspace: (projectId: string, checkpointId: string) =>
    invoke<CheckpointDiffSummary>("diff_checkpoint_with_workspace", {
      projectId,
      checkpointId,
    }),

  getFileDiff: (
    projectId: string,
    checkpointId: string,
    relPath: string,
    compareWithWorkspace: boolean,
    baseCheckpointId?: string,
  ) =>
    invoke<FileDiffResult>("get_file_diff", {
      projectId,
      checkpointId,
      relPath,
      baseCheckpointId: baseCheckpointId ?? null,
      compareWithWorkspace,
    }),

  getWorkspaceStatus: (projectId: string) =>
    invoke<WorkspaceStatus>("get_workspace_status", { projectId }),

  previewRestore: (
    projectId: string,
    checkpointId: string,
    selectedPaths: string[] | null,
    deleteExtraFiles: boolean,
  ) =>
    invoke<RestorePreview>("preview_restore", {
      projectId,
      checkpointId,
      selectedPaths,
      deleteExtraFiles,
    }),

  executeRestore: (request: ExecuteRestoreRequest) =>
    invoke<RestoreResult>("execute_restore", { request }),

  getStorageStats: () => invoke<StorageStats>("get_storage_stats"),
  runGc: () => invoke<GcReport>("run_gc"),
  applyRetention: (projectId: string) => invoke<GcReport>("apply_retention", { projectId }),
  verifyIntegrity: (projectId?: string) =>
    invoke<IntegrityReport>("verify_integrity", { projectId: projectId ?? null }),
};
