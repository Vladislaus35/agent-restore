import {
  GitBranch,
  Info,
  PlusCircle,
  ShieldCheck,
  SlidersHorizontal,
} from "lucide-react";
import type { TranslationDict } from "../i18n/translations";
import { formatBytes, formatPercent } from "../lib/format";
import type { Project, WorkspaceStatus } from "../lib/types";

interface ProjectHeaderProps {
  t: TranslationDict;
  project: Project;
  workspaceStatus: WorkspaceStatus | null;
  busy: boolean;
  onCreateCheckpoint: () => void;
  onVerifyIntegrity: () => void;
  onOpenSettings: () => void;
  onToggleAutoBackup: () => void;
}

export function ProjectHeader({
  t,
  project,
  workspaceStatus,
  busy,
  onCreateCheckpoint,
  onVerifyIntegrity,
  onOpenSettings,
  onToggleAutoBackup,
}: ProjectHeaderProps) {
  const savingsPct =
    project.logicalSizeBytes > 0
      ? ((project.logicalSizeBytes - project.storedSizeBytes) / project.logicalSizeBytes) * 100
      : 0;

  return (
    <header className="dashboard-header">
      <div className="dashboard-top-row">
        <div className="project-heading">
          <div className="project-title-line">
            <h1>{project.name}</h1>
            <button
              className={`badge ${project.autoBackup ? "badge-accent" : "badge-warn"}`}
              style={{ cursor: "pointer" }}
              onClick={onToggleAutoBackup}
            >
              {project.autoBackup ? t.header.autoOn : t.header.autoOff}
            </button>
            {project.gitBranch && (
              <span className="badge badge-neutral mono">
                <GitBranch size={11} />
                {project.gitBranch}
                {project.gitCommit ? ` · ${project.gitCommit}` : ""}
                {project.gitDirty ? " *" : ""}
              </span>
            )}
          </div>
          <div className="project-path mono">{project.rootPath}</div>
        </div>

        <div className="header-actions">
          <button className="btn btn-sm" onClick={onVerifyIntegrity} disabled={busy}>
            <ShieldCheck size={14} />
            {t.header.verify}
          </button>
          <button className="btn btn-sm" onClick={onOpenSettings} disabled={busy}>
            <SlidersHorizontal size={14} />
            {t.header.projectSettings}
          </button>
          <button className="btn btn-sm btn-primary" onClick={onCreateCheckpoint} disabled={busy}>
            <PlusCircle size={14} />
            {t.header.newCheckpoint}
          </button>
        </div>
      </div>

      <div className="stats-grid">
        <div className="stat-card">
          <span className="stat-label">{t.header.statCheckpoints}</span>
          <span className="stat-value">{project.checkpointCount}</span>
          <span className="stat-sub">
            max {project.maxCheckpoints} · {project.retentionDays}d
          </span>
        </div>

        <div className="stat-card">
          <span className="stat-label">{t.header.statTrackedFiles}</span>
          <span className="stat-value">{workspaceStatus?.trackedFileCount ?? "—"}</span>
          <span className="stat-sub">
            {workspaceStatus && workspaceStatus.untrackedFileCount > 0
              ? `${workspaceStatus.untrackedFileCount} ${t.header.untrackedLabel}`
              : "BLAKE3 verified"}
          </span>
        </div>

        <div className="stat-card">
          <span className="stat-label">{t.header.statPending}</span>
          {workspaceStatus?.hasPendingChanges ? (
            <>
              <span className="stat-value">
                <span className="delta-add">+{workspaceStatus.addedCount}</span>{" "}
                <span className="delta-mod">~{workspaceStatus.modifiedCount}</span>{" "}
                <span className="delta-del">-{workspaceStatus.deletedCount}</span>
              </span>
              <span className="stat-sub">
                debounce {project.debounceMs}ms
              </span>
            </>
          ) : (
            <>
              <span className="stat-value" style={{ color: "var(--accent)" }}>
                0
              </span>
              <span className="stat-sub">{t.header.cleanWorkspace}</span>
            </>
          )}
        </div>

        <div className="stat-card">
          <span className="stat-label">{t.header.statStorage}</span>
          <span className="stat-value">{formatBytes(project.storedSizeBytes)}</span>
          <span className="stat-sub">
            {formatBytes(project.logicalSizeBytes)} logical · -{formatPercent(savingsPct)}
          </span>
        </div>
      </div>

      <div className="transparency-bar">
        <Info size={13} style={{ flexShrink: 0 }} />
        <span>{t.header.transparencyNotice}</span>
      </div>
    </header>
  );
}
