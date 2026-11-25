import { useCallback, useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { AlertTriangle, FolderPlus, GitBranch, RotateCcw, ShieldCheck } from "lucide-react";
import { getTranslations } from "./i18n/translations";
import { api, parseBackendError } from "./lib/api";
import { formatBytes } from "./lib/format";
import type {
  AppMeta,
  Checkpoint,
  CheckpointDiffSummary,
  CheckpointFile,
  CrashRecoveryReport,
  ExclusionDefaults,
  FileDiffResult,
  GlobalSettings,
  MonitoringEvent,
  Project,
  StorageStats,
  ToastMessage,
  UpdateProjectRequest,
  WorkspaceStatus,
} from "./lib/types";
import { Sidebar, type ActivePage } from "./components/Sidebar";
import { ProjectHeader } from "./components/ProjectHeader";
import { TimelinePane } from "./components/TimelinePane";
import { CheckpointInspector, type InspectorMode } from "./components/CheckpointInspector";
import { RestoreModal } from "./components/RestoreModal";
import { CreateCheckpointModal } from "./components/CreateCheckpointModal";
import { ProjectSettingsModal } from "./components/ProjectSettingsModal";
import { StorageView } from "./components/StorageView";
import { SettingsView } from "./components/SettingsView";
import { ToastContainer } from "./components/ToastContainer";

const DEFAULT_SETTINGS: GlobalSettings = {
  language: "en",
  theme: "dark",
  minimizeToTray: true,
  autoStartMonitoring: true,
  defaultIntervalSecs: 180,
  defaultDebounceMs: 2500,
  defaultMaxStorageMb: 2048,
  defaultMaxCheckpoints: 200,
  defaultRetentionDays: 30,
  defaultMaxFileSizeMb: 50,
  minFreeDiskMb: 256,
  globalExcludes: [],
};

export function App() {
  const [settings, setSettings] = useState<GlobalSettings>(DEFAULT_SETTINGS);
  const [appMeta, setAppMeta] = useState<AppMeta>({
    version: "1.0.0",
    dataDir: "",
    monitoringPaused: false,
    platform: "windows",
  });
  const [exclusions, setExclusions] = useState<ExclusionDefaults | null>(null);
  const [recoveryReport, setRecoveryReport] = useState<CrashRecoveryReport | null>(null);
  const [activePage, setActivePage] = useState<ActivePage>("timeline");

  const [projects, setProjects] = useState<Project[]>([]);
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null);
  const [workspaceStatus, setWorkspaceStatus] = useState<WorkspaceStatus | null>(null);

  const [checkpoints, setCheckpoints] = useState<Checkpoint[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [triggerFilter, setTriggerFilter] = useState("all");
  const [selectedCheckpointId, setSelectedCheckpointId] = useState<string | null>(null);

  const [inspectorMode, setInspectorMode] = useState<InspectorMode>("parent");
  const [diffSummary, setDiffSummary] = useState<CheckpointDiffSummary | null>(null);
  const [allCheckpointFiles, setAllCheckpointFiles] = useState<CheckpointFile[]>([]);
  const [selectedFilePath, setSelectedFilePath] = useState<string | null>(null);
  const [fileDiff, setFileDiff] = useState<FileDiffResult | null>(null);
  const [diffLoading, setDiffLoading] = useState(false);

  const [storageStats, setStorageStats] = useState<StorageStats | null>(null);
  const [monitorState, setMonitorState] = useState<
    "monitoring" | "backing_up" | "paused" | "idle" | "error"
  >("monitoring");
  const [monitoringPaused, setMonitoringPaused] = useState(false);
  const [busy, setBusy] = useState(false);

  const [showCreateModal, setShowCreateModal] = useState(false);
  const [showProjectSettings, setShowProjectSettings] = useState(false);
  const [restoreTarget, setRestoreTarget] = useState<{
    checkpoint: Checkpoint;
    selectedPaths: string[] | null;
  } | null>(null);

  const [toasts, setToasts] = useState<ToastMessage[]>([]);

  const t = useMemo(() => getTranslations(settings.language), [settings.language]);

  const pushToast = useCallback(
    (kind: ToastMessage["kind"], title: string, detail?: string) => {
      const id = `${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
      setToasts((prev) => [...prev.slice(-4), { id, kind, title, detail }]);
      window.setTimeout(() => {
        setToasts((prev) => prev.filter((item) => item.id !== id));
      }, 5200);
    },
    [],
  );

  // Sync theme attribute on document root
  useEffect(() => {
    document.documentElement.setAttribute("data-theme", settings.theme);
  }, [settings.theme]);

  const refreshProjects = useCallback(
    async (preferProjectId?: string | null) => {
      try {
        const list = await api.listProjects();
        setProjects(list);
        setSelectedProjectId((curr) => {
          if (preferProjectId && list.some((p) => p.id === preferProjectId)) {
            return preferProjectId;
          }
          if (curr && list.some((p) => p.id === curr)) return curr;
          return list[0]?.id ?? null;
        });
      } catch (err) {
        pushToast("error", "Failed to load projects", parseBackendError(err).message);
      }
    },
    [pushToast],
  );

  const refreshStorage = useCallback(async () => {
    try {
      const stats = await api.getStorageStats();
      setStorageStats(stats);
    } catch (err) {
      pushToast("error", "Failed to load storage stats", parseBackendError(err).message);
    }
  }, [pushToast]);

  const refreshProjectTimeline = useCallback(
    async (projectId: string, q: string, tf: string, preferCpId?: string | null) => {
      try {
        const [cps, ws] = await Promise.all([
          api.listCheckpoints(projectId, q, tf, 300),
          api.getWorkspaceStatus(projectId).catch(() => null),
        ]);
        setCheckpoints(cps);
        setWorkspaceStatus(ws);
        setSelectedCheckpointId((curr) => {
          if (preferCpId && cps.some((c) => c.id === preferCpId)) return preferCpId;
          if (curr && cps.some((c) => c.id === curr)) return curr;
          return cps[0]?.id ?? null;
        });
      } catch (err) {
        pushToast("error", "Failed to load timeline", parseBackendError(err).message);
      }
    },
    [pushToast],
  );

  // Initial bootstrap
  useEffect(() => {
    (async () => {
      try {
        const [meta, loadedSettings, excl, crash] = await Promise.all([
          api.getAppMeta(),
          api.getSettings(),
          api.getDefaultExclusions(),
          api.getCrashRecoveryReport(),
        ]);
        setAppMeta(meta);
        setMonitoringPaused(meta.monitoringPaused);
        setMonitorState(meta.monitoringPaused ? "paused" : "monitoring");
        setSettings(loadedSettings);
        setExclusions(excl);
        if (crash.interruptedRestores.length > 0 || crash.cleanedIncompleteCheckpoints > 0) {
          setRecoveryReport(crash);
        }
        await refreshProjects();
      } catch (err) {
        pushToast("error", "Initialization error", parseBackendError(err).message);
      }
    })();
  }, [refreshProjects, pushToast]);

  // Listen to backend monitor events
  useEffect(() => {
    let unlistenMonitor: (() => void) | null = null;
    let unlistenRefresh: (() => void) | null = null;

    (async () => {
      unlistenMonitor = await listen<MonitoringEvent>("agent-restore://monitor", (event) => {
        const payload = event.payload;
        if (payload.state) {
          setMonitorState(payload.state);
          setMonitoringPaused(payload.state === "paused");
        }
        if (payload.kind === "checkpoint_created") {
          refreshProjects();
          if (payload.projectId && payload.projectId === selectedProjectId) {
            refreshProjectTimeline(
              payload.projectId,
              searchQuery,
              triggerFilter,
              payload.checkpointId,
            );
          }
        } else if (payload.kind === "changes_detected") {
          if (payload.projectId && payload.projectId === selectedProjectId) {
            api.getWorkspaceStatus(payload.projectId).then(setWorkspaceStatus).catch(() => {});
          }
        } else if (payload.kind === "error" && payload.message) {
          pushToast("error", t.status.error, payload.message);
        }
      });

      unlistenRefresh = await listen("agent-restore://refresh", () => {
        refreshProjects();
        if (selectedProjectId) {
          refreshProjectTimeline(selectedProjectId, searchQuery, triggerFilter);
        }
      });
    })();

    return () => {
      if (unlistenMonitor) unlistenMonitor();
      if (unlistenRefresh) unlistenRefresh();
    };
  }, [selectedProjectId, searchQuery, triggerFilter, refreshProjects, refreshProjectTimeline, pushToast, t.status.error]);

  // Load checkpoints when selected project or filters change
  useEffect(() => {
    if (!selectedProjectId) {
      setCheckpoints([]);
      setSelectedCheckpointId(null);
      setWorkspaceStatus(null);
      return;
    }
    refreshProjectTimeline(selectedProjectId, searchQuery, triggerFilter);
  }, [selectedProjectId, searchQuery, triggerFilter, refreshProjectTimeline]);

  // Load storage stats when opening storage page
  useEffect(() => {
    if (activePage === "storage") {
      refreshStorage();
    }
  }, [activePage, refreshStorage]);

  const selectedProject = useMemo(
    () => projects.find((p) => p.id === selectedProjectId) ?? null,
    [projects, selectedProjectId],
  );

  const selectedCheckpoint = useMemo(
    () => checkpoints.find((c) => c.id === selectedCheckpointId) ?? null,
    [checkpoints, selectedCheckpointId],
  );

  // Load checkpoint files & diff summary when selected checkpoint or inspector mode changes
  useEffect(() => {
    if (!selectedProjectId || !selectedCheckpointId) {
      setDiffSummary(null);
      setAllCheckpointFiles([]);
      setSelectedFilePath(null);
      setFileDiff(null);
      return;
    }

    let cancelled = false;
    (async () => {
      try {
        const [files, summary] = await Promise.all([
          api.getCheckpointFiles(selectedCheckpointId),
          inspectorMode === "workspace"
            ? api.diffCheckpointWithWorkspace(selectedProjectId, selectedCheckpointId)
            : api.diffCheckpoints(selectedCheckpointId),
        ]);
        if (cancelled) return;
        setAllCheckpointFiles(files);
        setDiffSummary(summary);
        const defaultPath =
          inspectorMode === "all"
            ? files[0]?.relPath ?? null
            : summary.changes[0]?.relPath ?? files[0]?.relPath ?? null;
        setSelectedFilePath((curr) => {
          if (curr && (files.some((f) => f.relPath === curr) || summary.changes.some((c) => c.relPath === curr))) {
            return curr;
          }
          return defaultPath;
        });
      } catch (err) {
        if (!cancelled) {
          pushToast("error", "Failed to inspect checkpoint", parseBackendError(err).message);
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [selectedProjectId, selectedCheckpointId, inspectorMode, pushToast]);

  // Load side-by-side diff for selected file
  useEffect(() => {
    if (!selectedProjectId || !selectedCheckpointId || !selectedFilePath) {
      setFileDiff(null);
      return;
    }

    let cancelled = false;
    setDiffLoading(true);
    api
      .getFileDiff(
        selectedProjectId,
        selectedCheckpointId,
        selectedFilePath,
        inspectorMode === "workspace",
      )
      .then((res) => {
        if (!cancelled) setFileDiff(res);
      })
      .catch((err) => {
        if (!cancelled) {
          setFileDiff(null);
          pushToast("error", "Diff failed", parseBackendError(err).message);
        }
      })
      .finally(() => {
        if (!cancelled) setDiffLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [selectedProjectId, selectedCheckpointId, selectedFilePath, inspectorMode, pushToast]);

  const handleAddProject = async () => {
    try {
      const folder = await api.pickProjectDirectory();
      if (!folder) return;
      setBusy(true);
      const created = await api.addProject({ rootPath: folder });
      await refreshProjects(created.id);
      setActivePage("timeline");
      pushToast("success", t.toasts.projectAdded, created.name);
    } catch (err) {
      pushToast("error", "Could not register project", parseBackendError(err).message);
    } finally {
      setBusy(false);
    }
  };

  const handleTogglePause = async () => {
    const next = !monitoringPaused;
    await api.setMonitoringPaused(next);
    setMonitoringPaused(next);
    setMonitorState(next ? "paused" : "monitoring");
  };

  const handleQuickSettingUpdate = async (patch: Partial<GlobalSettings>) => {
    const next = { ...settings, ...patch };
    setSettings(next);
    try {
      const saved = await api.saveSettings(next);
      setSettings(saved);
    } catch (err) {
      pushToast("error", "Failed to save settings", parseBackendError(err).message);
    }
  };

  const handleSaveSettings = async (next: GlobalSettings) => {
    try {
      const saved = await api.saveSettings(next);
      setSettings(saved);
      pushToast("success", t.settings.savedToast);
    } catch (err) {
      pushToast("error", "Invalid settings", parseBackendError(err).message);
    }
  };

  const handleCreateCheckpoint = async (label: string, note: string) => {
    if (!selectedProjectId) return;
    setBusy(true);
    try {
      const outcome = await api.createCheckpoint(selectedProjectId, label, note, true);
      if (outcome.checkpoint) {
        await refreshProjects(selectedProjectId);
        await refreshProjectTimeline(
          selectedProjectId,
          searchQuery,
          triggerFilter,
          outcome.checkpoint.id,
        );
        pushToast("success", t.toasts.checkpointCreated, outcome.checkpoint.label);
      } else if (outcome.skippedUnchanged) {
        pushToast("info", t.toasts.checkpointSkipped);
      }
      if (outcome.warnings.length > 0) {
        pushToast("warning", "Backup warnings", outcome.warnings[0]);
      }
    } catch (err) {
      pushToast("error", "Checkpoint failed", parseBackendError(err).message);
    } finally {
      setBusy(false);
    }
  };

  const handleVerifyIntegrity = async (projectId?: string) => {
    setBusy(true);
    try {
      const report = await api.verifyIntegrity(projectId);
      if (report.healthy) {
        pushToast(
          "success",
          t.toasts.integrityHealthy,
          `${report.checkedCheckpoints} checkpoints · ${report.checkedObjects} BLAKE3 objects`,
        );
      } else {
        pushToast(
          "error",
          t.toasts.integrityFailed,
          `${report.issues.length} issue(s): ${report.issues[0]?.relPath ?? ""}`,
        );
      }
    } catch (err) {
      pushToast("error", "Verification failed", parseBackendError(err).message);
    } finally {
      setBusy(false);
    }
  };

  const handleUpdateProject = async (req: UpdateProjectRequest) => {
    if (!selectedProjectId) return;
    try {
      await api.updateProject(selectedProjectId, req);
      await refreshProjects(selectedProjectId);
      pushToast("success", t.toasts.projectUpdated);
    } catch (err) {
      pushToast("error", "Update failed", parseBackendError(err).message);
    }
  };

  const handleRemoveProject = async () => {
    if (!selectedProjectId) return;
    try {
      const gc = await api.removeProject(selectedProjectId);
      await refreshProjects(null);
      pushToast(
        "success",
        t.toasts.projectRemoved,
        `Reclaimed ${formatBytes(gc.reclaimedBytes)}`,
      );
    } catch (err) {
      pushToast("error", "Remove failed", parseBackendError(err).message);
    }
  };

  const handleRenameCheckpoint = async (id: string, label: string, note?: string) => {
    try {
      await api.renameCheckpoint(id, label, note);
      if (selectedProjectId) {
        await refreshProjectTimeline(selectedProjectId, searchQuery, triggerFilter, id);
      }
      pushToast("success", t.toasts.checkpointRenamed);
    } catch (err) {
      pushToast("error", "Rename failed", parseBackendError(err).message);
    }
  };

  const handleDeleteCheckpoint = async (id: string) => {
    try {
      const gc = await api.deleteCheckpoint(id);
      if (selectedProjectId) {
        await refreshProjects(selectedProjectId);
        await refreshProjectTimeline(selectedProjectId, searchQuery, triggerFilter);
      }
      pushToast(
        "success",
        t.toasts.checkpointDeleted,
        `Reclaimed ${formatBytes(gc.reclaimedBytes)}`,
      );
    } catch (err) {
      pushToast("error", "Delete failed", parseBackendError(err).message);
    }
  };

  const handleRunGc = async () => {
    setBusy(true);
    try {
      const gc = await api.runGc();
      await refreshStorage();
      await refreshProjects();
      pushToast(
        "success",
        t.toasts.gcComplete,
        `${gc.removedObjects} objects · ${formatBytes(gc.reclaimedBytes)} reclaimed`,
      );
    } catch (err) {
      pushToast("error", "GC failed", parseBackendError(err).message);
    } finally {
      setBusy(false);
    }
  };

  const handleApplyRetention = async (projectId: string) => {
    setBusy(true);
    try {
      const gc = await api.applyRetention(projectId);
      await refreshStorage();
      await refreshProjects();
      pushToast(
        "success",
        t.toasts.gcComplete,
        `${gc.removedCheckpoints} checkpoints · ${formatBytes(gc.reclaimedBytes)} reclaimed`,
      );
    } catch (err) {
      pushToast("error", "Retention failed", parseBackendError(err).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="app-shell">
      <Sidebar
        t={t}
        lang={settings.language}
        theme={settings.theme}
        activePage={activePage}
        onSelectPage={setActivePage}
        projects={projects}
        selectedProjectId={selectedProjectId}
        onSelectProject={(id) => {
          setSelectedProjectId(id);
          setSelectedCheckpointId(null);
          setSelectedFilePath(null);
        }}
        onAddProject={handleAddProject}
        monitorState={monitorState}
        monitoringPaused={monitoringPaused}
        onTogglePause={handleTogglePause}
        onToggleTheme={() =>
          handleQuickSettingUpdate({ theme: settings.theme === "dark" ? "light" : "dark" })
        }
        onToggleLang={() =>
          handleQuickSettingUpdate({ language: settings.language === "en" ? "tr" : "en" })
        }
        appVersion={appMeta.version}
      />

      <main className="main-content">
        {recoveryReport &&
          (recoveryReport.interruptedRestores.length > 0 ||
            recoveryReport.cleanedIncompleteCheckpoints > 0) && (
            <div className="recovery-banner">
              <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                <AlertTriangle size={16} color="var(--warn)" />
                <div>
                  <strong style={{ fontSize: 13 }}>{t.recovery.title}</strong>
                  <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
                    {recoveryReport.interruptedRestores[0]?.message ?? t.recovery.cleanedSummary}
                  </div>
                </div>
              </div>
              <div style={{ display: "flex", gap: 8 }}>
                {recoveryReport.interruptedRestores[0]?.recoveryCheckpointId && (
                  <button
                    className="btn btn-xs btn-primary"
                    onClick={() => {
                      const notice = recoveryReport.interruptedRestores[0];
                      setSelectedProjectId(notice.projectId);
                      setSelectedCheckpointId(notice.recoveryCheckpointId);
                      setActivePage("timeline");
                    }}
                  >
                    {t.recovery.openRecoveryCp}
                  </button>
                )}
                <button
                  className="btn btn-xs"
                  onClick={async () => {
                    await api.dismissCrashRecoveryNotices();
                    setRecoveryReport(null);
                  }}
                >
                  {t.recovery.dismiss}
                </button>
              </div>
            </div>
          )}

        {activePage === "storage" ? (
          <StorageView
            t={t}
            stats={storageStats}
            busy={busy}
            onRefresh={refreshStorage}
            onRunGc={handleRunGc}
            onVerifyAll={() => handleVerifyIntegrity(undefined)}
            onApplyRetention={handleApplyRetention}
          />
        ) : activePage === "settings" ? (
          <SettingsView
            t={t}
            settings={settings}
            exclusions={exclusions}
            appVersion={appMeta.version}
            dataDir={appMeta.dataDir}
            onSave={handleSaveSettings}
          />
        ) : !selectedProject ? (
          <div className="empty-state">
            <div className="brand-mark" style={{ width: 52, height: 52, borderRadius: 12 }}>
              <svg width="32" height="32" viewBox="0 0 24 24" fill="none">
                <path
                  d="M4.5 10.5A8 8 0 1 1 6.3 17.6"
                  stroke="#10b981"
                  strokeWidth="2.2"
                  strokeLinecap="round"
                />
                <polygon points="3,11.5 8.2,11.1 5.2,6.4" fill="#10b981" />
                <circle cx="12" cy="12" r="2.2" fill="#f8fafc" />
              </svg>
            </div>
            <h1 style={{ margin: 0, fontSize: 22 }}>{t.empty.title}</h1>
            <p style={{ margin: 0, maxWidth: 560, color: "var(--text-secondary)" }}>
              {t.empty.subtitle}
            </p>
            <button className="btn btn-primary" onClick={handleAddProject} disabled={busy}>
              <FolderPlus size={15} />
              {t.empty.cta}
            </button>

            <div className="empty-features">
              <div className="feature-card">
                <ShieldCheck size={18} color="var(--accent)" />
                <strong>{t.empty.feature1Title}</strong>
                <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>
                  {t.empty.feature1Desc}
                </span>
              </div>
              <div className="feature-card">
                <GitBranch size={18} color="var(--info)" />
                <strong>{t.empty.feature2Title}</strong>
                <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>
                  {t.empty.feature2Desc}
                </span>
              </div>
              <div className="feature-card">
                <RotateCcw size={18} color="var(--warn)" />
                <strong>{t.empty.feature3Title}</strong>
                <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>
                  {t.empty.feature3Desc}
                </span>
              </div>
            </div>
          </div>
        ) : (
          <>
            <ProjectHeader
              t={t}
              project={selectedProject}
              workspaceStatus={workspaceStatus}
              busy={busy}
              onCreateCheckpoint={() => setShowCreateModal(true)}
              onVerifyIntegrity={() => handleVerifyIntegrity(selectedProject.id)}
              onOpenSettings={() => setShowProjectSettings(true)}
              onToggleAutoBackup={() =>
                handleUpdateProject({ autoBackup: !selectedProject.autoBackup })
              }
            />

            <div className="workspace-split">
              <TimelinePane
                t={t}
                lang={settings.language}
                checkpoints={checkpoints}
                selectedCheckpointId={selectedCheckpointId}
                searchQuery={searchQuery}
                triggerFilter={triggerFilter}
                onSearchChange={setSearchQuery}
                onTriggerFilterChange={setTriggerFilter}
                onSelectCheckpoint={(cp) => setSelectedCheckpointId(cp.id)}
                onRenameCheckpoint={handleRenameCheckpoint}
                onDeleteCheckpoint={handleDeleteCheckpoint}
              />

              <CheckpointInspector
                t={t}
                lang={settings.language}
                checkpoint={selectedCheckpoint}
                mode={inspectorMode}
                onModeChange={setInspectorMode}
                diffSummary={diffSummary}
                allFiles={allCheckpointFiles}
                selectedFilePath={selectedFilePath}
                onSelectFile={setSelectedFilePath}
                fileDiff={fileDiff}
                diffLoading={diffLoading}
                onRequestRestoreFull={(cp) =>
                  setRestoreTarget({ checkpoint: cp, selectedPaths: null })
                }
                onRequestRestoreFile={(cp, relPath) =>
                  setRestoreTarget({ checkpoint: cp, selectedPaths: [relPath] })
                }
              />
            </div>
          </>
        )}
      </main>

      {showCreateModal && (
        <CreateCheckpointModal
          t={t}
          onClose={() => setShowCreateModal(false)}
          onCreate={handleCreateCheckpoint}
        />
      )}

      {showProjectSettings && selectedProject && (
        <ProjectSettingsModal
          t={t}
          project={selectedProject}
          onClose={() => setShowProjectSettings(false)}
          onSave={handleUpdateProject}
          onRemove={handleRemoveProject}
        />
      )}

      {restoreTarget && selectedProject && (
        <RestoreModal
          t={t}
          projectId={selectedProject.id}
          checkpoint={restoreTarget.checkpoint}
          selectedPaths={restoreTarget.selectedPaths}
          onClose={() => setRestoreTarget(null)}
          onRestored={async (result) => {
            setRestoreTarget(null);
            await refreshProjects(selectedProject.id);
            await refreshProjectTimeline(
              selectedProject.id,
              searchQuery,
              triggerFilter,
              result.preRestoreCheckpoint.id,
            );
            pushToast(
              "success",
              t.toasts.restoreSuccess,
              `${result.restoredCount} restored, ${result.deletedCount} removed (safety checkpoint created)`,
            );
          }}
        />
      )}

      <ToastContainer
        toasts={toasts}
        onDismiss={(id) => setToasts((prev) => prev.filter((item) => item.id !== id))}
      />
    </div>
  );
}

export default App;
