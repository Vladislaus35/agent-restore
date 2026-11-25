import { useState } from "react";
import { Columns2, FileCode2, Rows3, RotateCcw } from "lucide-react";
import type { TranslationDict } from "../i18n/translations";
import type { FileDiffResult } from "../lib/types";

interface DiffViewerProps {
  t: TranslationDict;
  diff: FileDiffResult | null;
  loading: boolean;
  canRestoreFile: boolean;
  onRestoreSelectedFile: () => void;
}

export function DiffViewer({
  t,
  diff,
  loading,
  canRestoreFile,
  onRestoreSelectedFile,
}: DiffViewerProps) {
  const [mode, setMode] = useState<"split" | "unified">("split");

  if (loading) {
    return (
      <div className="diff-pane">
        <div className="empty-state" style={{ color: "var(--text-muted)" }}>
          Loading diff…
        </div>
      </div>
    );
  }

  if (!diff) {
    return (
      <div className="diff-pane">
        <div className="empty-state" style={{ color: "var(--text-muted)" }}>
          <FileCode2 size={28} strokeWidth={1.5} />
          <span>{t.inspector.selectFilePrompt}</span>
        </div>
      </div>
    );
  }

  return (
    <div className="diff-pane">
      <div className="diff-toolbar">
        <div style={{ display: "flex", alignItems: "center", gap: 10, minWidth: 0 }}>
          <span
            className="mono"
            style={{
              fontWeight: 600,
              fontSize: 12.5,
              whiteSpace: "nowrap",
              overflow: "hidden",
              textOverflow: "ellipsis",
            }}
          >
            {diff.relPath}
          </span>
          {!diff.isBinary && (
            <span className="delta-chips">
              <span className="delta-add">+{diff.insertions}</span>
              <span className="delta-del">-{diff.deletions}</span>
            </span>
          )}
        </div>

        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <div className="inspector-tabs">
            <button
              className={`tab-btn ${mode === "split" ? "active" : ""}`}
              onClick={() => setMode("split")}
              title={t.inspector.sideBySide}
            >
              <Columns2 size={12} style={{ marginRight: 4, verticalAlign: -1 }} />
              {t.inspector.sideBySide}
            </button>
            <button
              className={`tab-btn ${mode === "unified" ? "active" : ""}`}
              onClick={() => setMode("unified")}
              title={t.inspector.unified}
            >
              <Rows3 size={12} style={{ marginRight: 4, verticalAlign: -1 }} />
              {t.inspector.unified}
            </button>
          </div>

          {canRestoreFile && (
            <button className="btn btn-xs btn-primary" onClick={onRestoreSelectedFile}>
              <RotateCcw size={12} />
              {t.inspector.restoreFile}
            </button>
          )}
        </div>
      </div>

      {diff.truncated && (
        <div className="alert alert-warn" style={{ margin: "8px 12px 0", padding: "6px 10px" }}>
          {t.inspector.truncatedNotice}
        </div>
      )}

      {diff.isBinary ? (
        <div className="empty-state" style={{ color: "var(--text-muted)" }}>
          <span>{t.inspector.binaryNotice}</span>
        </div>
      ) : diff.rows.length === 0 ? (
        <div className="empty-state" style={{ color: "var(--text-muted)" }}>
          <span>{t.inspector.identicalFile}</span>
        </div>
      ) : mode === "split" ? (
        <div className="diff-scroll">
          <table className="diff-table mono">
            <thead>
              <tr>
                <th style={{ width: 46 }}>#</th>
                <th>{diff.leftLabel}</th>
                <th style={{ width: 46 }}>#</th>
                <th>{diff.rightLabel}</th>
              </tr>
            </thead>
            <tbody>
              {diff.rows.map((row, idx) => (
                <tr key={idx}>
                  <td
                    className={`diff-gutter ${
                      row.leftKind === "delete" ? "cell-del-gutter" : ""
                    }`}
                  >
                    {row.leftLineNo ?? ""}
                  </td>
                  <td
                    className={`diff-code diff-divider ${
                      row.leftKind === "delete" ? "cell-del" : ""
                    }`}
                  >
                    {row.leftKind === "delete" ? "- " : row.leftText !== null ? "  " : ""}
                    {row.leftText ?? ""}
                  </td>
                  <td
                    className={`diff-gutter ${
                      row.rightKind === "insert" ? "cell-add-gutter" : ""
                    }`}
                  >
                    {row.rightLineNo ?? ""}
                  </td>
                  <td
                    className={`diff-code ${
                      row.rightKind === "insert" ? "cell-add" : ""
                    }`}
                  >
                    {row.rightKind === "insert" ? "+ " : row.rightText !== null ? "  " : ""}
                    {row.rightText ?? ""}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="diff-scroll">
          <table className="diff-table mono">
            <thead>
              <tr>
                <th style={{ width: 46 }}>Old</th>
                <th style={{ width: 46 }}>New</th>
                <th>
                  {diff.leftLabel} → {diff.rightLabel}
                </th>
              </tr>
            </thead>
            <tbody>
              {diff.rows.flatMap((row, idx) => {
                if (row.leftKind === "equal") {
                  return [
                    <tr key={`eq-${idx}`}>
                      <td className="diff-gutter">{row.leftLineNo}</td>
                      <td className="diff-gutter">{row.rightLineNo}</td>
                      <td className="diff-code">  {row.leftText}</td>
                    </tr>,
                  ];
                }
                const out = [];
                if (row.leftKind === "delete") {
                  out.push(
                    <tr key={`del-${idx}`}>
                      <td className="diff-gutter cell-del-gutter">{row.leftLineNo}</td>
                      <td className="diff-gutter cell-del-gutter"></td>
                      <td className="diff-code cell-del">- {row.leftText}</td>
                    </tr>,
                  );
                }
                if (row.rightKind === "insert") {
                  out.push(
                    <tr key={`ins-${idx}`}>
                      <td className="diff-gutter cell-add-gutter"></td>
                      <td className="diff-gutter cell-add-gutter">{row.rightLineNo}</td>
                      <td className="diff-code cell-add">+ {row.rightText}</td>
                    </tr>,
                  );
                }
                return out;
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
