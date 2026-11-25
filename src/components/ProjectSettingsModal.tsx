import { useState } from "react";
import { Trash2, X } from "lucide-react";
import type { TranslationDict } from "../i18n/translations";
import type { Project, UpdateProjectRequest } from "../lib/types";

interface ProjectSettingsModalProps {
  t: TranslationDict;
  project: Project;
  onClose: () => void;
  onSave: (req: UpdateProjectRequest) => Promise<void>;
  onRemove: () => Promise<void>;
}

export function ProjectSettingsModal({
  t,
  project,
  onClose,
  onSave,
  onRemove,
}: ProjectSettingsModalProps) {
  const [name, setName] = useState(project.name);
  const [enabled, setEnabled] = useState(project.enabled);
  const [autoBackup, setAutoBackup] = useState(project.autoBackup);
  const [intervalSecs, setIntervalSecs] = useState(project.intervalSecs);
  const [debounceMs, setDebounceMs] = useState(project.debounceMs);
  const [maxStorageMb, setMaxStorageMb] = useState(project.maxStorageMb);
  const [maxCheckpoints, setMaxCheckpoints] = useState(project.maxCheckpoints);
  const [retentionDays, setRetentionDays] = useState(project.retentionDays);
  const [maxFileSizeMb, setMaxFileSizeMb] = useState(project.maxFileSizeMb);
  const [respectGitignore, setRespectGitignore] = useState(project.respectGitignore);
  const [gitIntegration, setGitIntegration] = useState(project.gitIntegration);
  const [excludesText, setExcludesText] = useState(project.customExcludes.join("\n"));
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [saving, setSaving] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setSaving(true);
    try {
      const customExcludes = excludesText
        .split("\n")
        .map((l) => l.trim())
        .filter(Boolean);
      await onSave({
        name: name.trim() || project.name,
        enabled,
        autoBackup,
        intervalSecs: Number(intervalSecs) || 180,
        debounceMs: Number(debounceMs) || 2500,
        maxStorageMb: Number(maxStorageMb) || 2048,
        maxCheckpoints: Number(maxCheckpoints) || 200,
        retentionDays: Number(retentionDays) || 30,
        maxFileSizeMb: Number(maxFileSizeMb) || 50,
        respectGitignore,
        gitIntegration,
        customExcludes,
      });
      onClose();
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal-card" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2>{t.projectSettings.title}</h2>
          <button className="btn btn-xs btn-ghost" onClick={onClose}>
            <X size={16} />
          </button>
        </div>

        <form onSubmit={handleSubmit}>
          <div className="modal-body">
            <div className="field">
              <label>{t.projectSettings.name}</label>
              <input className="input" value={name} onChange={(e) => setName(e.target.value)} />
            </div>

            <div className="field">
              <label>{t.projectSettings.rootPath}</label>
              <input className="input mono" value={project.rootPath} disabled />
            </div>

            <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
              <label className="checkbox-row">
                <input
                  type="checkbox"
                  checked={enabled}
                  onChange={(e) => setEnabled(e.target.checked)}
                />
                <span>{t.projectSettings.enabled}</span>
              </label>
              <label className="checkbox-row">
                <input
                  type="checkbox"
                  checked={autoBackup}
                  onChange={(e) => setAutoBackup(e.target.checked)}
                />
                <span>{t.projectSettings.autoBackup}</span>
              </label>
              <label className="checkbox-row">
                <input
                  type="checkbox"
                  checked={respectGitignore}
                  onChange={(e) => setRespectGitignore(e.target.checked)}
                />
                <span>{t.projectSettings.respectGitignore}</span>
              </label>
              <label className="checkbox-row">
                <input
                  type="checkbox"
                  checked={gitIntegration}
                  onChange={(e) => setGitIntegration(e.target.checked)}
                />
                <span>{t.projectSettings.gitIntegration}</span>
              </label>
            </div>

            <div className="form-grid">
              <div className="field">
                <label>{t.projectSettings.intervalSecs}</label>
                <input
                  type="number"
                  min={10}
                  className="input"
                  value={intervalSecs}
                  onChange={(e) => setIntervalSecs(Number(e.target.value))}
                />
              </div>
              <div className="field">
                <label>{t.projectSettings.debounceMs}</label>
                <input
                  type="number"
                  min={300}
                  step={100}
                  className="input"
                  value={debounceMs}
                  onChange={(e) => setDebounceMs(Number(e.target.value))}
                />
              </div>
              <div className="field">
                <label>{t.projectSettings.maxStorageMb}</label>
                <input
                  type="number"
                  min={16}
                  className="input"
                  value={maxStorageMb}
                  onChange={(e) => setMaxStorageMb(Number(e.target.value))}
                />
              </div>
              <div className="field">
                <label>{t.projectSettings.maxCheckpoints}</label>
                <input
                  type="number"
                  min={2}
                  className="input"
                  value={maxCheckpoints}
                  onChange={(e) => setMaxCheckpoints(Number(e.target.value))}
                />
              </div>
              <div className="field">
                <label>{t.projectSettings.retentionDays}</label>
                <input
                  type="number"
                  min={1}
                  className="input"
                  value={retentionDays}
                  onChange={(e) => setRetentionDays(Number(e.target.value))}
                />
              </div>
              <div className="field">
                <label>{t.projectSettings.maxFileSizeMb}</label>
                <input
                  type="number"
                  min={1}
                  className="input"
                  value={maxFileSizeMb}
                  onChange={(e) => setMaxFileSizeMb(Number(e.target.value))}
                />
              </div>
            </div>

            <div className="field">
              <label>{t.projectSettings.customExcludes}</label>
              <textarea
                className="textarea mono"
                rows={4}
                value={excludesText}
                onChange={(e) => setExcludesText(e.target.value)}
                placeholder={t.projectSettings.customExcludesHint}
              />
            </div>

            {confirmRemove && (
              <div className="alert alert-danger" style={{ flexDirection: "column", gap: 8 }}>
                <span>{t.projectSettings.removeConfirm}</span>
                <div style={{ display: "flex", gap: 8 }}>
                  <button
                    type="button"
                    className="btn btn-xs btn-danger"
                    onClick={async () => {
                      await onRemove();
                      onClose();
                    }}
                  >
                    {t.projectSettings.removeProject}
                  </button>
                  <button
                    type="button"
                    className="btn btn-xs"
                    onClick={() => setConfirmRemove(false)}
                  >
                    {t.projectSettings.cancel}
                  </button>
                </div>
              </div>
            )}
          </div>

          <div className="modal-footer">
            <button
              type="button"
              className="btn btn-sm btn-ghost"
              style={{ color: "var(--danger)" }}
              onClick={() => setConfirmRemove(true)}
            >
              <Trash2 size={13} />
              {t.projectSettings.removeProject}
            </button>
            <div style={{ display: "flex", gap: 8 }}>
              <button type="button" className="btn btn-sm" onClick={onClose} disabled={saving}>
                {t.projectSettings.cancel}
              </button>
              <button type="submit" className="btn btn-sm btn-primary" disabled={saving}>
                {t.projectSettings.save}
              </button>
            </div>
          </div>
        </form>
      </div>
    </div>
  );
}
