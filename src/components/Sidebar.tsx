import {
  FolderPlus,
  HardDrive,
  History,
  Pause,
  Play,
  Settings,
  ShieldCheck,
  Sun,
  Moon,
} from "lucide-react";
import type { TranslationDict } from "../i18n/translations";
import { formatRelativeTime } from "../lib/format";
import type { Language, Project, Theme } from "../lib/types";

export type ActivePage = "timeline" | "storage" | "settings";

interface SidebarProps {
  t: TranslationDict;
  lang: Language;
  theme: Theme;
  activePage: ActivePage;
  onSelectPage: (page: ActivePage) => void;
  projects: Project[];
  selectedProjectId: string | null;
  onSelectProject: (id: string) => void;
  onAddProject: () => void;
  monitorState: "monitoring" | "backing_up" | "paused" | "idle" | "error";
  monitoringPaused: boolean;
  onTogglePause: () => void;
  onToggleTheme: () => void;
  onToggleLang: () => void;
  appVersion: string;
}

export function Sidebar({
  t,
  lang,
  theme,
  activePage,
  onSelectPage,
  projects,
  selectedProjectId,
  onSelectProject,
  onAddProject,
  monitorState,
  monitoringPaused,
  onTogglePause,
  onToggleTheme,
  onToggleLang,
  appVersion,
}: SidebarProps) {
  const effectiveState = monitoringPaused ? "paused" : monitorState;
  const statusLabel = t.status[effectiveState] ?? t.status.monitoring;

  return (
    <aside className="sidebar">
      <div className="sidebar-brand">
        <div className="brand-mark" aria-hidden="true">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none">
            <path
              d="M4.5 10.5A8 8 0 1 1 6.3 17.6"
              stroke="#10b981"
              strokeWidth="2.3"
              strokeLinecap="round"
            />
            <polygon points="3,11.5 8.2,11.1 5.2,6.4" fill="#10b981" />
            <circle cx="12" cy="12" r="2.2" fill="#f8fafc" />
          </svg>
        </div>
        <div>
          <div className="brand-title">{t.appName}</div>
          <div className="brand-sub">{t.tagline}</div>
        </div>
      </div>

      <div className="status-bar-card">
        <div className="status-dot-wrap">
          <span className={`status-dot ${effectiveState}`} />
          <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
            {statusLabel}
          </span>
        </div>
        <button
          className="btn btn-xs btn-ghost"
          onClick={onTogglePause}
          title={monitoringPaused ? t.status.resume : t.status.pause}
        >
          {monitoringPaused ? <Play size={12} /> : <Pause size={12} />}
          {monitoringPaused ? t.status.resume : t.status.pause}
        </button>
      </div>

      <div className="sidebar-section-header">
        <span>{t.sidebar.projects}</span>
        <button className="btn btn-xs btn-ghost" onClick={onAddProject} title={t.sidebar.addProject}>
          <FolderPlus size={13} />
          {t.sidebar.addProject}
        </button>
      </div>

      <div className="project-list">
        {projects.length === 0 ? (
          <div style={{ padding: "12px 6px", fontSize: 12, color: "var(--text-muted)" }}>
            {t.sidebar.noProjects}
          </div>
        ) : (
          projects.map((p) => {
            const isSelected = p.id === selectedProjectId && activePage === "timeline";
            return (
              <button
                key={p.id}
                className={`project-item ${isSelected ? "active" : ""}`}
                onClick={() => {
                  onSelectProject(p.id);
                  onSelectPage("timeline");
                }}
              >
                <div className="project-item-top">
                  <span className="project-item-name">{p.name}</span>
                  <span
                    className={`status-dot ${
                      !p.enabled || !p.autoBackup || monitoringPaused ? "paused" : "monitoring"
                    }`}
                  />
                </div>
                <div className="project-item-meta">
                  <span>
                    {p.checkpointCount} {t.sidebar.checkpointsShort}
                  </span>
                  <span>{formatRelativeTime(p.lastCheckpointAt, lang)}</span>
                </div>
              </button>
            );
          })
        )}
      </div>

      <nav className="sidebar-nav">
        <button
          className={`nav-btn ${activePage === "timeline" ? "active" : ""}`}
          onClick={() => onSelectPage("timeline")}
        >
          <History size={15} />
          {t.nav.timeline}
        </button>
        <button
          className={`nav-btn ${activePage === "storage" ? "active" : ""}`}
          onClick={() => onSelectPage("storage")}
        >
          <HardDrive size={15} />
          {t.nav.storage}
        </button>
        <button
          className={`nav-btn ${activePage === "settings" ? "active" : ""}`}
          onClick={() => onSelectPage("settings")}
        >
          <Settings size={15} />
          {t.nav.settings}
        </button>
      </nav>

      <div className="sidebar-footer">
        <span className="badge badge-neutral" style={{ gap: 4 }}>
          <ShieldCheck size={11} />
          {t.status.offlineBadge}
        </span>
        <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
          <button className="btn btn-xs btn-ghost" onClick={onToggleLang} title="Switch language">
            {lang.toUpperCase()}
          </button>
          <button className="btn btn-xs btn-ghost" onClick={onToggleTheme} title="Toggle theme">
            {theme === "dark" ? <Sun size={13} /> : <Moon size={13} />}
          </button>
          <span className="mono" style={{ fontSize: 10.5 }}>
            v{appVersion}
          </span>
        </div>
      </div>
    </aside>
  );
}
