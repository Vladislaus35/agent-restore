import { useMemo, useState } from "react";
import { RotateCcw, Search } from "lucide-react";
import type { TranslationDict } from "../i18n/translations";
import { formatBytes, formatTimestamp } from "../lib/format";
import type {
  ChangeKind,
  Checkpoint,
  CheckpointDiffSummary,
  CheckpointFile,
  FileDiffResult,
  Language,
} from "../lib/types";
import { DiffViewer } from "./DiffViewer";

export type InspectorMode = "parent" | "workspace" | "all";

interface CheckpointInspectorProps {
  t: TranslationDict;
  lang: Language;
  checkpoint: Checkpoint | null;
  mode: InspectorMode;
  onModeChange: (mode: InspectorMode) => void;
  diffSummary: CheckpointDiffSummary | null;
  allFiles: CheckpointFile[];
  selectedFilePath: string | null;
  onSelectFile: (relPath: string) => void;
  fileDiff: FileDiffResult | null;
  diffLoading: boolean;
  onRequestRestoreFull: (checkpoint: Checkpoint) => void;
  onRequestRestoreFile: (checkpoint: Checkpoint, relPath: string) => void;
}

interface DisplayFileItem {
  relPath: string;
  kind: ChangeKind;
  sizeBytes: number;
  isBinary: boolean;
  gitStatus: string;
  existsInCheckpoint: boolean;
}

function kindCode(kind: ChangeKind): string {
  switch (kind) {
    case "added":
      return "A";
    case "modified":
      return "M";
    case "deleted":
      return "D";
    default:
      return "F";
  }
}

export function CheckpointInspector({
  t,
  lang,
  checkpoint,
  mode,
  onModeChange,
  diffSummary,
  allFiles,
  selectedFilePath,
  onSelectFile,
  fileDiff,
  diffLoading,
  onRequestRestoreFull,
  onRequestRestoreFile,
}: CheckpointInspectorProps) {
  const [fileFilter, setFileFilter] = useState("");

  const checkpointFileSet = useMemo(
    () => new Set(allFiles.map((f) => f.relPath)),
    [allFiles],
  );

  const items = useMemo<DisplayFileItem[]>(() => {
    let list: DisplayFileItem[];
    if (mode === "all") {
      list = allFiles.map((f) => ({
        relPath: f.relPath,
        kind: "unchanged",
        sizeBytes: f.sizeBytes,
        isBinary: f.isBinary,
        gitStatus: f.gitStatus,
        existsInCheckpoint: true,
      }));
    } else {
      list = (diffSummary?.changes ?? []).map((c) => ({
        relPath: c.relPath,
        kind: c.kind,
        sizeBytes: c.newSize ?? c.oldSize ?? 0,
        isBinary: c.isBinary,
        gitStatus: c.gitStatus,
        existsInCheckpoint: checkpointFileSet.has(c.relPath),
      }));
    }

    const q = fileFilter.trim().toLowerCase();
    if (!q) return list;
    return list.filter((it) => it.relPath.toLowerCase().includes(q));
  }, [mode, allFiles, diffSummary, fileFilter, checkpointFileSet]);

  if (!checkpoint) {
    return (
      <section className="inspector-pane">
        <div className="empty-state" style={{ color: "var(--text-muted)" }}>
          <span>{t.inspector.selectPrompt}</span>
        </div>
      </section>
    );
  }

  const canRestoreSelected =
    selectedFilePath !== null && checkpointFileSet.has(selectedFilePath);

  return (
    <section className="inspector-pane">
      <div className="inspector-header">
        <div style={{ display: "flex", flexDirection: "column", gap: 3, minWidth: 0 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
            <strong style={{ fontSize: 14 }}>{checkpoint.label}</strong>
            <span className="mono" style={{ fontSize: 11, color: "var(--text-muted)" }}>
              {t.inspector.treeHash}: {checkpoint.treeHash.slice(0, 12)}
            </span>
          </div>
          <div style={{ fontSize: 11.5, color: "var(--text-muted)" }}>
            {formatTimestamp(checkpoint.createdAt, lang)}
            {checkpoint.gitBranch ? ` · ${checkpoint.gitBranch}` : ""}
            {checkpoint.gitCommit ? ` (${checkpoint.gitCommit})` : ""}
          </div>
        </div>

        <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
          <div className="inspector-tabs">
            <button
              className={`tab-btn ${mode === "parent" ? "active" : ""}`}
              onClick={() => onModeChange("parent")}
            >
              {t.inspector.compareParent}
            </button>
            <button
              className={`tab-btn ${mode === "workspace" ? "active" : ""}`}
              onClick={() => onModeChange("workspace")}
            >
              {t.inspector.compareWorkspace}
            </button>
            <button
              className={`tab-btn ${mode === "all" ? "active" : ""}`}
              onClick={() => onModeChange("all")}
            >
              {t.inspector.allFiles} ({allFiles.length})
            </button>
          </div>

          <button
            className="btn btn-sm btn-primary"
            onClick={() => onRequestRestoreFull(checkpoint)}
          >
            <RotateCcw size={13} />
            {t.inspector.restoreFull}
          </button>
        </div>
      </div>

      <div className="inspector-body">
        <div className="file-list-pane">
          <div style={{ padding: 8, borderBottom: "1px solid var(--border)" }}>
            <div className="search-box">
              <Search size={13} />
              <input
                className="search-input"
                value={fileFilter}
                onChange={(e) => setFileFilter(e.target.value)}
                placeholder={t.inspector.filterFiles}
              />
            </div>
          </div>

          <div className="file-list-scroll">
            {items.length === 0 ? (
              <div
                style={{
                  padding: "20px 10px",
                  textAlign: "center",
                  fontSize: 12,
                  color: "var(--text-muted)",
                }}
              >
                {t.inspector.noChanges}
              </div>
            ) : (
              items.map((item) => {
                const isSelected = item.relPath === selectedFilePath;
                return (
                  <div
                    key={item.relPath}
                    className={`file-row ${isSelected ? "selected" : ""}`}
                    onClick={() => onSelectFile(item.relPath)}
                    title={item.relPath}
                  >
                    <div className="file-row-left">
                      <span className={`file-kind-pill ${item.kind}`}>
                        {kindCode(item.kind)}
                      </span>
                      <div style={{ minWidth: 0, flex: 1 }}>
                        <div className="file-path-text mono">{item.relPath}</div>
                        <div
                          style={{
                            display: "flex",
                            alignItems: "center",
                            gap: 5,
                            fontSize: 10.5,
                            color: "var(--text-muted)",
                          }}
                        >
                          <span>{formatBytes(item.sizeBytes)}</span>
                          {item.gitStatus === "untracked" && (
                            <span style={{ color: "var(--info)" }}>
                              · {t.inspector.untrackedBadge}
                            </span>
                          )}
                          {item.isBinary && <span>· {t.inspector.binaryBadge}</span>}
                        </div>
                      </div>
                    </div>

                    {item.existsInCheckpoint && (
                      <button
                        className="btn btn-xs btn-ghost"
                        title={t.inspector.restoreFile}
                        onClick={(e) => {
                          e.stopPropagation();
                          onRequestRestoreFile(checkpoint, item.relPath);
                        }}
                      >
                        <RotateCcw size={12} />
                      </button>
                    )}
                  </div>
                );
              })
            )}
          </div>
        </div>

        <DiffViewer
          t={t}
          diff={fileDiff}
          loading={diffLoading}
          canRestoreFile={canRestoreSelected}
          onRestoreSelectedFile={() => {
            if (selectedFilePath) {
              onRequestRestoreFile(checkpoint, selectedFilePath);
            }
          }}
        />
      </div>
    </section>
  );
}
