import { useState } from "react";
import { Check, Pencil, Search, Trash2, X } from "lucide-react";
import type { TranslationDict } from "../i18n/translations";
import { formatBytes, formatRelativeTime, formatTimestamp } from "../lib/format";
import type { Checkpoint, Language, TriggerType } from "../lib/types";

interface TimelinePaneProps {
  t: TranslationDict;
  lang: Language;
  checkpoints: Checkpoint[];
  selectedCheckpointId: string | null;
  searchQuery: string;
  triggerFilter: string;
  onSearchChange: (q: string) => void;
  onTriggerFilterChange: (f: string) => void;
  onSelectCheckpoint: (cp: Checkpoint) => void;
  onRenameCheckpoint: (id: string, label: string, note?: string) => Promise<void>;
  onDeleteCheckpoint: (id: string) => Promise<void>;
}

function triggerBadgeClass(trigger: TriggerType): string {
  switch (trigger) {
    case "manual":
      return "badge-info";
    case "pre_restore":
      return "badge-warn";
    case "watcher":
      return "badge-accent";
    default:
      return "badge-neutral";
  }
}

function triggerLabel(t: TranslationDict, trigger: TriggerType): string {
  switch (trigger) {
    case "manual":
      return t.timeline.filterManual;
    case "watcher":
      return t.timeline.filterWatcher;
    case "auto":
      return t.timeline.filterAuto;
    case "pre_restore":
      return t.timeline.filterPreRestore;
  }
}

export function TimelinePane({
  t,
  lang,
  checkpoints,
  selectedCheckpointId,
  searchQuery,
  triggerFilter,
  onSearchChange,
  onTriggerFilterChange,
  onSelectCheckpoint,
  onRenameCheckpoint,
  onDeleteCheckpoint,
}: TimelinePaneProps) {
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editLabel, setEditLabel] = useState("");
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);

  const filters = [
    { id: "all", label: t.timeline.filterAll },
    { id: "manual", label: t.timeline.filterManual },
    { id: "watcher", label: t.timeline.filterWatcher },
    { id: "auto", label: t.timeline.filterAuto },
    { id: "pre_restore", label: t.timeline.filterPreRestore },
  ];

  const handleSaveRename = async (id: string) => {
    if (!editLabel.trim()) return;
    await onRenameCheckpoint(id, editLabel.trim());
    setEditingId(null);
  };

  return (
    <section className="timeline-pane">
      <div className="timeline-toolbar">
        <div className="search-box">
          <Search size={14} />
          <input
            className="search-input"
            value={searchQuery}
            onChange={(e) => onSearchChange(e.target.value)}
            placeholder={t.timeline.searchPlaceholder}
          />
        </div>
        <div className="filter-pills">
          {filters.map((f) => (
            <button
              key={f.id}
              className={`pill ${triggerFilter === f.id ? "active" : ""}`}
              onClick={() => onTriggerFilterChange(f.id)}
            >
              {f.label}
            </button>
          ))}
        </div>
      </div>

      <div className="timeline-list">
        {checkpoints.length === 0 ? (
          <div style={{ padding: "24px 12px", textAlign: "center", color: "var(--text-muted)" }}>
            {t.timeline.emptySearch}
          </div>
        ) : (
          checkpoints.map((cp) => {
            const isSelected = cp.id === selectedCheckpointId;
            const isEditing = editingId === cp.id;
            const isDeleting = confirmDeleteId === cp.id;

            return (
              <div
                key={cp.id}
                className={`timeline-card ${isSelected ? "selected" : ""}`}
                onClick={() => {
                  if (!isEditing) onSelectCheckpoint(cp);
                }}
              >
                <div className="timeline-card-top">
                  {isEditing ? (
                    <div
                      style={{ display: "flex", gap: 4, flex: 1 }}
                      onClick={(e) => e.stopPropagation()}
                    >
                      <input
                        className="input"
                        style={{ padding: "3px 6px", fontSize: 12.5 }}
                        value={editLabel}
                        autoFocus
                        onChange={(e) => setEditLabel(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") handleSaveRename(cp.id);
                          if (e.key === "Escape") setEditingId(null);
                        }}
                      />
                      <button
                        className="btn btn-xs btn-primary"
                        onClick={() => handleSaveRename(cp.id)}
                        title={t.timeline.save}
                      >
                        <Check size={12} />
                      </button>
                      <button
                        className="btn btn-xs btn-ghost"
                        onClick={() => setEditingId(null)}
                        title={t.timeline.cancel}
                      >
                        <X size={12} />
                      </button>
                    </div>
                  ) : (
                    <>
                      <span className="timeline-label">{cp.label}</span>
                      <div
                        style={{ display: "flex", alignItems: "center", gap: 2, flexShrink: 0 }}
                        onClick={(e) => e.stopPropagation()}
                      >
                        <button
                          className="btn btn-xs btn-ghost"
                          onClick={() => {
                            setEditingId(cp.id);
                            setEditLabel(cp.label);
                            setConfirmDeleteId(null);
                          }}
                          title={t.timeline.rename}
                        >
                          <Pencil size={11} />
                        </button>
                        <button
                          className="btn btn-xs btn-ghost"
                          onClick={() =>
                            setConfirmDeleteId(isDeleting ? null : cp.id)
                          }
                          title={t.timeline.delete}
                        >
                          <Trash2 size={11} />
                        </button>
                      </div>
                    </>
                  )}
                </div>

                {cp.note && (
                  <div style={{ fontSize: 11.5, color: "var(--text-secondary)" }}>{cp.note}</div>
                )}

                <div className="timeline-meta">
                  <span className={`badge ${triggerBadgeClass(cp.triggerType)}`}>
                    {triggerLabel(t, cp.triggerType)}
                  </span>
                  <span title={formatTimestamp(cp.createdAt, lang)}>
                    {formatRelativeTime(cp.createdAt, lang)}
                  </span>
                </div>

                <div className="timeline-meta">
                  <div className="delta-chips">
                    <span className="delta-add">+{cp.addedCount}</span>
                    <span className="delta-mod">~{cp.modifiedCount}</span>
                    <span className="delta-del">-{cp.deletedCount}</span>
                  </div>
                  <span>
                    {cp.fileCount} {t.timeline.filesCount} · {formatBytes(cp.totalSizeBytes)}
                  </span>
                </div>

                {isDeleting && (
                  <div
                    className="alert alert-danger"
                    style={{ padding: "7px 9px", marginTop: 4, flexDirection: "column", gap: 6 }}
                    onClick={(e) => e.stopPropagation()}
                  >
                    <span style={{ fontSize: 11.5 }}>{t.timeline.deleteConfirm}</span>
                    <div style={{ display: "flex", gap: 6 }}>
                      <button
                        className="btn btn-xs btn-danger"
                        onClick={async () => {
                          setConfirmDeleteId(null);
                          await onDeleteCheckpoint(cp.id);
                        }}
                      >
                        {t.timeline.delete}
                      </button>
                      <button
                        className="btn btn-xs"
                        onClick={() => setConfirmDeleteId(null)}
                      >
                        {t.timeline.cancel}
                      </button>
                    </div>
                  </div>
                )}
              </div>
            );
          })
        )}
      </div>
    </section>
  );
}
