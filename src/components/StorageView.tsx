import { AlertTriangle, RefreshCw, ShieldCheck, Trash2 } from "lucide-react";
import type { TranslationDict } from "../i18n/translations";
import { formatBytes, formatPercent } from "../lib/format";
import type { StorageStats } from "../lib/types";

interface StorageViewProps {
  t: TranslationDict;
  stats: StorageStats | null;
  busy: boolean;
  onRefresh: () => void;
  onRunGc: () => void;
  onVerifyAll: () => void;
  onApplyRetention: (projectId: string) => void;
}

export function StorageView({
  t,
  stats,
  busy,
  onRefresh,
  onRunGc,
  onVerifyAll,
  onApplyRetention,
}: StorageViewProps) {
  if (!stats) {
    return (
      <div className="empty-state" style={{ color: "var(--text-muted)" }}>
        Loading storage statistics…
      </div>
    );
  }

  return (
    <div className="page-scroll">
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", gap: 12, flexWrap: "wrap" }}>
        <div>
          <h1 style={{ margin: 0, fontSize: 20 }}>{t.storage.title}</h1>
          <p style={{ margin: "4px 0 0", color: "var(--text-secondary)" }}>{t.storage.subtitle}</p>
        </div>
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
          <button className="btn btn-sm" onClick={onRefresh} disabled={busy}>
            <RefreshCw size={13} />
          </button>
          <button className="btn btn-sm" onClick={onVerifyAll} disabled={busy}>
            <ShieldCheck size={13} />
            {t.storage.verifyAll}
          </button>
          <button className="btn btn-sm btn-primary" onClick={onRunGc} disabled={busy}>
            <Trash2 size={13} />
            {t.storage.runGc}
          </button>
        </div>
      </div>

      {stats.diskSpaceLow && (
        <div className="alert alert-danger">
          <AlertTriangle size={16} />
          <span>{t.storage.lowDiskWarning}</span>
        </div>
      )}

      <div className="stats-grid">
        <div className="stat-card">
          <span className="stat-label">{t.storage.storedSize}</span>
          <span className="stat-value">{formatBytes(stats.totalStoredBytes)}</span>
          <span className="stat-sub">
            {stats.objectCount} {t.storage.objectsCount}
          </span>
        </div>
        <div className="stat-card">
          <span className="stat-label">{t.storage.logicalSize}</span>
          <span className="stat-value">{formatBytes(stats.totalLogicalBytes)}</span>
          <span className="stat-sub">Uncompressed snapshot sum</span>
        </div>
        <div className="stat-card">
          <span className="stat-label">{t.storage.savings}</span>
          <span className="stat-value" style={{ color: "var(--accent)" }}>
            {formatPercent(stats.dedupSavingsPercent)}
          </span>
          <span className="stat-sub">{formatBytes(stats.dedupSavingsBytes)} saved</span>
        </div>
        <div className="stat-card">
          <span className="stat-label">{t.storage.availableDisk}</span>
          <span
            className="stat-value"
            style={{ color: stats.diskSpaceLow ? "var(--danger)" : undefined }}
          >
            {formatBytes(stats.availableDiskBytes)}
          </span>
          <span className="stat-sub">
            Safety floor: {formatBytes(stats.minFreeDiskBytes)}
          </span>
        </div>
      </div>

      <div className="section-card">
        <h2>{t.storage.dataDir}</h2>
        <div className="mono" style={{ fontSize: 12.5, color: "var(--text-secondary)", wordBreak: "break-all" }}>
          {stats.dataDir}
        </div>
      </div>

      <div className="section-card">
        <h2>{t.storage.perProject}</h2>
        {stats.projects.length === 0 ? (
          <div style={{ color: "var(--text-muted)" }}>{t.storage.noProjects}</div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
            {stats.projects.map((p) => {
              const limitBytes = p.maxStorageMb * 1024 * 1024;
              const pct = limitBytes > 0 ? Math.min(100, (p.storedSizeBytes / limitBytes) * 100) : 0;
              const barClass = pct >= 90 ? "danger" : pct >= 70 ? "warn" : "";

              return (
                <div key={p.projectId} style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                  <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 10 }}>
                    <div>
                      <strong>{p.projectName}</strong>
                      <span style={{ color: "var(--text-muted)", marginLeft: 8, fontSize: 12 }}>
                        {p.checkpointCount} checkpoints
                      </span>
                    </div>
                    <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                      <span style={{ fontSize: 12.5 }}>
                        <strong>{formatBytes(p.storedSizeBytes)}</strong> / {p.maxStorageMb} MB{" "}
                        <span style={{ color: "var(--text-muted)" }}>
                          ({formatBytes(p.logicalSizeBytes)} logical)
                        </span>
                      </span>
                      <button
                        className="btn btn-xs"
                        disabled={busy}
                        onClick={() => onApplyRetention(p.projectId)}
                      >
                        {t.storage.applyRetention}
                      </button>
                    </div>
                  </div>
                  <div className="progress-track">
                    <div className={`progress-fill ${barClass}`} style={{ width: `${Math.max(2, pct)}%` }} />
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
