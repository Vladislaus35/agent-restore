import { useEffect, useState } from "react";
import { AlertTriangle, RefreshCw, RotateCcw, ShieldAlert, ShieldCheck, X } from "lucide-react";
import type { TranslationDict } from "../i18n/translations";
import { api, parseBackendError } from "../lib/api";
import { formatBytes } from "../lib/format";
import type { Checkpoint, RestorePreview, RestoreResult } from "../lib/types";

interface RestoreModalProps {
  t: TranslationDict;
  projectId: string;
  checkpoint: Checkpoint;
  selectedPaths: string[] | null;
  onClose: () => void;
  onRestored: (result: RestoreResult) => void;
}

export function RestoreModal({
  t,
  projectId,
  checkpoint,
  selectedPaths,
  onClose,
  onRestored,
}: RestoreModalProps) {
  const isFull = selectedPaths === null;
  const [deleteExtra, setDeleteExtra] = useState(false);
  const [preview, setPreview] = useState<RestorePreview | null>(null);
  const [loading, setLoading] = useState(true);
  const [restoring, setRestoring] = useState(false);
  const [confirmed, setConfirmed] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [conflictPaths, setConflictPaths] = useState<string[] | null>(null);

  const loadPreview = async (delExtra: boolean) => {
    setLoading(true);
    setErrorMsg(null);
    setConflictPaths(null);
    setConfirmed(false);
    try {
      const p = await api.previewRestore(projectId, checkpoint.id, selectedPaths, delExtra);
      setPreview(p);
    } catch (err) {
      setErrorMsg(parseBackendError(err).message);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadPreview(deleteExtra);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [projectId, checkpoint.id, deleteExtra]);

  const totalAffected = preview
    ? preview.filesToRecreate.length +
      preview.filesToOverwrite.length +
      preview.extraFilesToDelete.length
    : 0;

  const isDestructive =
    preview !== null &&
    (preview.filesToOverwrite.length > 0 || preview.extraFilesToDelete.length > 0);

  const handleConfirmRestore = async () => {
    if (!preview || !preview.integrityOk) return;
    setRestoring(true);
    setErrorMsg(null);
    setConflictPaths(null);
    try {
      const result = await api.executeRestore({
        projectId,
        checkpointId: checkpoint.id,
        selectedPaths,
        deleteExtraFiles: isFull && deleteExtra,
        expectedFingerprint: preview.workspaceFingerprint,
        confirmed: true,
      });
      onRestored(result);
    } catch (err) {
      const parsed = parseBackendError(err);
      if (parsed.code === "CONCURRENT_MODIFICATION") {
        const paths = (parsed.details?.paths as string[] | undefined) ?? [];
        setConflictPaths(paths);
      } else {
        setErrorMsg(parsed.message);
      }
    } finally {
      setRestoring(false);
    }
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal-card" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <div>
            <h2>{isFull ? t.restoreModal.titleFull : t.restoreModal.titleFiles}</h2>
            <div style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 2 }}>
              {checkpoint.label}
              {!isFull && selectedPaths ? ` · ${selectedPaths.join(", ")}` : ""}
            </div>
          </div>
          <button className="btn btn-xs btn-ghost" onClick={onClose}>
            <X size={16} />
          </button>
        </div>

        <div className="modal-body">
          <div className="alert alert-accent">
            <ShieldCheck size={16} style={{ flexShrink: 0, marginTop: 1, color: "var(--accent)" }} />
            <span>{t.restoreModal.safetyBanner}</span>
          </div>

          {errorMsg && (
            <div className="alert alert-danger">
              <ShieldAlert size={16} style={{ flexShrink: 0, marginTop: 1 }} />
              <span>{errorMsg}</span>
            </div>
          )}

          {conflictPaths && (
            <div className="alert alert-warn" style={{ flexDirection: "column", gap: 6 }}>
              <div style={{ display: "flex", alignItems: "center", gap: 8, fontWeight: 600 }}>
                <AlertTriangle size={15} />
                {t.restoreModal.conflictTitle}
              </div>
              <span>{t.restoreModal.conflictDesc}</span>
              <button className="btn btn-xs" onClick={() => loadPreview(deleteExtra)}>
                <RefreshCw size={12} />
                {t.restoreModal.refreshPreview}
              </button>
            </div>
          )}

          {loading ? (
            <div style={{ padding: 24, textAlign: "center", color: "var(--text-muted)" }}>
              Scanning workspace & verifying BLAKE3 hashes…
            </div>
          ) : preview ? (
            <>
              {preview.integrityOk ? (
                <div style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12, color: "var(--accent)" }}>
                  <ShieldCheck size={14} />
                  <span>{t.restoreModal.integrityOk}</span>
                  <span style={{ color: "var(--text-muted)", marginLeft: "auto" }}>
                    {formatBytes(preview.totalBytesToWrite)}
                  </span>
                </div>
              ) : (
                <div className="alert alert-danger" style={{ flexDirection: "column", gap: 4 }}>
                  <strong>{t.restoreModal.integrityFailed}</strong>
                  {preview.corruptedObjects.map((c, idx) => (
                    <span key={idx} className="mono" style={{ fontSize: 11.5 }}>
                      {c}
                    </span>
                  ))}
                </div>
              )}

              {isFull && (preview.extraFilesKept.length > 0 || preview.extraFilesToDelete.length > 0) && (
                <div
                  style={{
                    border: "1px solid var(--border)",
                    borderRadius: 7,
                    padding: 10,
                    display: "flex",
                    flexDirection: "column",
                    gap: 8,
                    background: "var(--bg-surface-2)",
                  }}
                >
                  <label className="checkbox-row">
                    <input
                      type="radio"
                      name="extraPolicy"
                      checked={!deleteExtra}
                      onChange={() => setDeleteExtra(false)}
                    />
                    <span>{t.restoreModal.keepExtra}</span>
                  </label>
                  <label className="checkbox-row" style={{ color: deleteExtra ? "var(--danger)" : undefined }}>
                    <input
                      type="radio"
                      name="extraPolicy"
                      checked={deleteExtra}
                      onChange={() => setDeleteExtra(true)}
                    />
                    <span>{t.restoreModal.deleteExtra}</span>
                  </label>
                </div>
              )}

              {totalAffected === 0 ? (
                <div className="alert" style={{ background: "var(--bg-surface-2)" }}>
                  {t.restoreModal.noDifferences}
                </div>
              ) : (
                <>
                  {preview.filesToRecreate.length > 0 && (
                    <div className="field">
                      <label style={{ color: "var(--accent)" }}>
                        {t.restoreModal.recreateTitle} ({preview.filesToRecreate.length})
                      </label>
                      <div className="restore-file-list mono" style={{ fontSize: 12 }}>
                        {preview.filesToRecreate.map((f) => (
                          <div
                            key={f.relPath}
                            style={{ display: "flex", justifyContent: "space-between", gap: 8 }}
                          >
                            <span>+ {f.relPath}</span>
                            <span style={{ color: "var(--text-muted)" }}>
                              {formatBytes(f.targetSize ?? 0)}
                            </span>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}

                  {preview.filesToOverwrite.length > 0 && (
                    <div className="field">
                      <label style={{ color: "var(--warn)" }}>
                        {t.restoreModal.overwriteTitle} ({preview.filesToOverwrite.length})
                      </label>
                      <div className="restore-file-list mono" style={{ fontSize: 12 }}>
                        {preview.filesToOverwrite.map((f) => (
                          <div
                            key={f.relPath}
                            style={{ display: "flex", justifyContent: "space-between", gap: 8 }}
                          >
                            <span>~ {f.relPath}</span>
                            <span style={{ color: "var(--text-muted)" }}>
                              {formatBytes(f.currentSize ?? 0)} → {formatBytes(f.targetSize ?? 0)}
                            </span>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}

                  {preview.extraFilesToDelete.length > 0 && (
                    <div className="field">
                      <label style={{ color: "var(--danger)" }}>
                        {t.restoreModal.extraTitle} ({preview.extraFilesToDelete.length})
                      </label>
                      <div className="restore-file-list mono" style={{ fontSize: 12 }}>
                        {preview.extraFilesToDelete.map((f) => (
                          <div
                            key={f.relPath}
                            style={{ display: "flex", justifyContent: "space-between", gap: 8 }}
                          >
                            <span>- {f.relPath}</span>
                            <span style={{ color: "var(--text-muted)" }}>
                              {formatBytes(f.currentSize ?? 0)}
                            </span>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}

                  <div style={{ fontSize: 12, color: "var(--text-muted)" }}>
                    {preview.unchangedCount} {t.restoreModal.unchangedCount}
                  </div>

                  {isDestructive && (
                    <label
                      className="checkbox-row"
                      style={{
                        padding: 10,
                        borderRadius: 6,
                        border: "1px solid var(--border-strong)",
                        background: "var(--bg-surface-2)",
                      }}
                    >
                      <input
                        type="checkbox"
                        checked={confirmed}
                        onChange={(e) => setConfirmed(e.target.checked)}
                      />
                      <span style={{ fontSize: 12.5 }}>{t.restoreModal.confirmCheckbox}</span>
                    </label>
                  )}
                </>
              )}
            </>
          ) : null}
        </div>

        <div className="modal-footer">
          <button className="btn btn-sm" onClick={() => loadPreview(deleteExtra)} disabled={loading || restoring}>
            <RefreshCw size={13} />
            {t.restoreModal.refreshPreview}
          </button>
          <div style={{ display: "flex", gap: 8 }}>
            <button className="btn btn-sm" onClick={onClose} disabled={restoring}>
              {t.restoreModal.cancel}
            </button>
            <button
              className={`btn btn-sm ${deleteExtra && (preview?.extraFilesToDelete.length ?? 0) > 0 ? "btn-danger" : "btn-primary"}`}
              disabled={
                loading ||
                restoring ||
                !preview ||
                !preview.integrityOk ||
                totalAffected === 0 ||
                (isDestructive && !confirmed)
              }
              onClick={handleConfirmRestore}
            >
              <RotateCcw size={13} />
              {restoring ? t.restoreModal.restoring : t.restoreModal.confirmBtn}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
