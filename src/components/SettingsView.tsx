import { useState } from "react";
import { Save, ShieldCheck } from "lucide-react";
import type { TranslationDict } from "../i18n/translations";
import type { ExclusionDefaults, GlobalSettings, Language, Theme } from "../lib/types";

interface SettingsViewProps {
  t: TranslationDict;
  settings: GlobalSettings;
  exclusions: ExclusionDefaults | null;
  appVersion: string;
  dataDir: string;
  onSave: (next: GlobalSettings) => Promise<void>;
}

export function SettingsView({
  t,
  settings,
  exclusions,
  appVersion,
  dataDir,
  onSave,
}: SettingsViewProps) {
  const [draft, setDraft] = useState<GlobalSettings>(settings);
  const [globalExcludesText, setGlobalExcludesText] = useState(
    settings.globalExcludes.join("\n"),
  );
  const [saving, setSaving] = useState(false);

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    setSaving(true);
    try {
      const globalExcludes = globalExcludesText
        .split("\n")
        .map((l) => l.trim())
        .filter(Boolean);
      await onSave({ ...draft, globalExcludes });
    } finally {
      setSaving(false);
    }
  };

  return (
    <form className="page-scroll" onSubmit={handleSave}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", gap: 12 }}>
        <div>
          <h1 style={{ margin: 0, fontSize: 20 }}>{t.settings.title}</h1>
          <p style={{ margin: "4px 0 0", color: "var(--text-secondary)" }}>{t.settings.subtitle}</p>
        </div>
        <button type="submit" className="btn btn-primary" disabled={saving}>
          <Save size={14} />
          {t.settings.save}
        </button>
      </div>

      <div className="section-card">
        <h2>{t.settings.general}</h2>
        <div className="form-grid">
          <div className="field">
            <label>{t.settings.language}</label>
            <select
              className="select"
              value={draft.language}
              onChange={(e) => setDraft({ ...draft, language: e.target.value as Language })}
            >
              <option value="en">English</option>
              <option value="tr">Türkçe</option>
            </select>
          </div>

          <div className="field">
            <label>{t.settings.theme}</label>
            <select
              className="select"
              value={draft.theme}
              onChange={(e) => setDraft({ ...draft, theme: e.target.value as Theme })}
            >
              <option value="dark">{t.settings.themeDark}</option>
              <option value="light">{t.settings.themeLight}</option>
            </select>
          </div>
        </div>

        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={draft.minimizeToTray}
            onChange={(e) => setDraft({ ...draft, minimizeToTray: e.target.checked })}
          />
          <span>{t.settings.minimizeToTray}</span>
        </label>

        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={draft.autoStartMonitoring}
            onChange={(e) => setDraft({ ...draft, autoStartMonitoring: e.target.checked })}
          />
          <span>{t.settings.autoStartMonitoring}</span>
        </label>
      </div>

      <div className="section-card">
        <h2>{t.settings.backupDefaults}</h2>
        <div className="form-grid">
          <div className="field">
            <label>{t.settings.intervalSecs}</label>
            <input
              type="number"
              min={10}
              className="input"
              value={draft.defaultIntervalSecs}
              onChange={(e) =>
                setDraft({ ...draft, defaultIntervalSecs: Number(e.target.value) || 180 })
              }
            />
          </div>
          <div className="field">
            <label>{t.settings.debounceMs}</label>
            <input
              type="number"
              min={300}
              step={100}
              className="input"
              value={draft.defaultDebounceMs}
              onChange={(e) =>
                setDraft({ ...draft, defaultDebounceMs: Number(e.target.value) || 2500 })
              }
            />
          </div>
          <div className="field">
            <label>{t.settings.maxStorageMb}</label>
            <input
              type="number"
              min={16}
              className="input"
              value={draft.defaultMaxStorageMb}
              onChange={(e) =>
                setDraft({ ...draft, defaultMaxStorageMb: Number(e.target.value) || 2048 })
              }
            />
          </div>
          <div className="field">
            <label>{t.settings.maxCheckpoints}</label>
            <input
              type="number"
              min={2}
              className="input"
              value={draft.defaultMaxCheckpoints}
              onChange={(e) =>
                setDraft({ ...draft, defaultMaxCheckpoints: Number(e.target.value) || 200 })
              }
            />
          </div>
          <div className="field">
            <label>{t.settings.retentionDays}</label>
            <input
              type="number"
              min={1}
              className="input"
              value={draft.defaultRetentionDays}
              onChange={(e) =>
                setDraft({ ...draft, defaultRetentionDays: Number(e.target.value) || 30 })
              }
            />
          </div>
          <div className="field">
            <label>{t.settings.maxFileSizeMb}</label>
            <input
              type="number"
              min={1}
              className="input"
              value={draft.defaultMaxFileSizeMb}
              onChange={(e) =>
                setDraft({ ...draft, defaultMaxFileSizeMb: Number(e.target.value) || 50 })
              }
            />
          </div>
          <div className="field">
            <label>{t.settings.minFreeDiskMb}</label>
            <input
              type="number"
              min={32}
              className="input"
              value={draft.minFreeDiskMb}
              onChange={(e) =>
                setDraft({ ...draft, minFreeDiskMb: Number(e.target.value) || 256 })
              }
            />
          </div>
        </div>
      </div>

      <div className="section-card">
        <h2>{t.settings.exclusions}</h2>
        {exclusions && (
          <div className="form-grid">
            <div className="field">
              <label>{t.settings.immutableSecurity}</label>
              <div className="restore-file-list mono" style={{ fontSize: 11.5 }}>
                {exclusions.security.map((rule, i) => (
                  <div key={i}>{rule}</div>
                ))}
              </div>
            </div>
            <div className="field">
              <label>{t.settings.defaultCaches}</label>
              <div className="restore-file-list mono" style={{ fontSize: 11.5 }}>
                {exclusions.caches.map((rule, i) => (
                  <div key={i}>{rule}</div>
                ))}
              </div>
            </div>
          </div>
        )}

        <div className="field">
          <label>{t.settings.globalCustom}</label>
          <textarea
            className="textarea mono"
            rows={4}
            value={globalExcludesText}
            onChange={(e) => setGlobalExcludesText(e.target.value)}
            placeholder="*.sqlite&#10;scratch/**"
          />
        </div>
      </div>

      <div className="section-card">
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <ShieldCheck size={18} color="var(--accent)" />
          <h2>{t.settings.offlineTitle}</h2>
        </div>
        <p style={{ margin: 0, color: "var(--text-secondary)" }}>{t.settings.offlineDesc}</p>
        <div style={{ fontSize: 12, color: "var(--text-muted)" }} className="mono">
          Agent Restore v{appVersion} · MIT License · Store: {dataDir}
        </div>
      </div>
    </form>
  );
}
