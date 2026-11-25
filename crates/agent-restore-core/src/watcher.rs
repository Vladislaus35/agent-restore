use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::engine::AgentRestoreEngine;
use crate::ignore::IgnoreMatcher;
use crate::models::{CreateCheckpointOptions, MonitoringEvent, Project, TriggerType};
use crate::security::path_to_safe_rel;

struct WatchedProjectState {
    project: Project,
    canonical_root: PathBuf,
    matcher: IgnoreMatcher,
    _watcher: RecommendedWatcher,
    dirty_since: Option<Instant>,
    last_event_at: Option<Instant>,
    last_backup_at: Instant,
}

enum ControlMsg {
    ReloadProjects,
    SetPaused(bool),
    SuspendProject(String),
    ResumeProject(String),
    Shutdown,
}

pub struct MonitorService {
    paused: Arc<AtomicBool>,
    tx: mpsc::Sender<ControlMsg>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl MonitorService {
    pub fn start<F>(engine: Arc<AgentRestoreEngine>, on_event: F) -> Self
    where
        F: Fn(MonitoringEvent) + Send + Sync + 'static,
    {
        let initial_paused = engine
            .get_settings()
            .map(|s| !s.auto_start_monitoring)
            .unwrap_or(false);
        let paused = Arc::new(AtomicBool::new(initial_paused));
        let paused_clone = paused.clone();
        let (ctrl_tx, ctrl_rx) = mpsc::channel::<ControlMsg>();
        let (fs_tx, fs_rx) = mpsc::channel::<(String, Event)>();

        let handle = thread::Builder::new()
            .name("agent-restore-monitor".to_string())
            .spawn(move || {
                let mut watched: HashMap<String, WatchedProjectState> = HashMap::new();
                let mut suspended: HashSet<String> = HashSet::new();

                let reload = |watched: &mut HashMap<String, WatchedProjectState>| {
                    watched.clear();
                    let settings = match engine.get_settings() {
                        Ok(s) => s,
                        Err(_) => return,
                    };
                    let projects = match engine.list_projects() {
                        Ok(p) => p,
                        Err(_) => return,
                    };

                    for project in projects {
                        if !project.enabled || !project.auto_backup {
                            continue;
                        }
                        let root = Path::new(&project.root_path);
                        if !root.is_dir() {
                            continue;
                        }
                        let matcher = match IgnoreMatcher::build(
                            root,
                            &settings.global_excludes,
                            &project.custom_excludes,
                            project.respect_gitignore,
                            project.max_file_size_mb,
                        ) {
                            Ok(m) => m,
                            Err(_) => continue,
                        };

                        let pid = project.id.clone();
                        let sender = fs_tx.clone();
                        let watcher_res = RecommendedWatcher::new(
                            move |res: notify::Result<Event>| {
                                if let Ok(ev) = res {
                                    let _ = sender.send((pid.clone(), ev));
                                }
                            },
                            Config::default(),
                        );

                        if let Ok(mut watcher) = watcher_res {
                            if watcher.watch(root, RecursiveMode::Recursive).is_ok() {
                                watched.insert(
                                    project.id.clone(),
                                    WatchedProjectState {
                                        canonical_root: root.to_path_buf(),
                                        project,
                                        matcher,
                                        _watcher: watcher,
                                        dirty_since: None,
                                        last_event_at: None,
                                        last_backup_at: Instant::now(),
                                    },
                                );
                            }
                        }
                    }
                };

                reload(&mut watched);
                on_event(MonitoringEvent {
                    kind: "status".to_string(),
                    project_id: None,
                    state: Some(if paused_clone.load(Ordering::Relaxed) {
                        "paused".to_string()
                    } else {
                        "monitoring".to_string()
                    }),
                    checkpoint_id: None,
                    message: None,
                });

                loop {
                    // Drain control messages
                    while let Ok(msg) = ctrl_rx.try_recv() {
                        match msg {
                            ControlMsg::ReloadProjects => {
                                reload(&mut watched);
                            }
                            ControlMsg::SetPaused(p) => {
                                paused_clone.store(p, Ordering::Relaxed);
                                on_event(MonitoringEvent {
                                    kind: "status".to_string(),
                                    project_id: None,
                                    state: Some(if p {
                                        "paused".to_string()
                                    } else {
                                        "monitoring".to_string()
                                    }),
                                    checkpoint_id: None,
                                    message: None,
                                });
                            }
                            ControlMsg::SuspendProject(pid) => {
                                suspended.insert(pid);
                            }
                            ControlMsg::ResumeProject(pid) => {
                                suspended.remove(&pid);
                                if let Some(st) = watched.get_mut(&pid) {
                                    st.dirty_since = None;
                                    st.last_event_at = None;
                                    st.last_backup_at = Instant::now();
                                }
                            }
                            ControlMsg::Shutdown => {
                                watched.clear();
                                return;
                            }
                        }
                    }

                    // Drain filesystem events with short wait to keep idle CPU near 0%
                    match fs_rx.recv_timeout(Duration::from_millis(350)) {
                        Ok((pid, ev)) => {
                            if paused_clone.load(Ordering::Relaxed) || suspended.contains(&pid) {
                                continue;
                            }
                            if !is_relevant_event_kind(&ev.kind) {
                                continue;
                            }
                            if let Some(st) = watched.get_mut(&pid) {
                                let has_eligible_path = ev.paths.iter().any(|p| {
                                    if let Ok(rel) = path_to_safe_rel(&st.canonical_root, p) {
                                        !st.matcher.is_excluded(&rel, p.is_dir())
                                    } else {
                                        false
                                    }
                                });
                                if has_eligible_path {
                                    let now = Instant::now();
                                    let was_clean = st.dirty_since.is_none();
                                    if was_clean {
                                        st.dirty_since = Some(now);
                                        on_event(MonitoringEvent {
                                            kind: "changes_detected".to_string(),
                                            project_id: Some(pid.clone()),
                                            state: Some("monitoring".to_string()),
                                            checkpoint_id: None,
                                            message: None,
                                        });
                                    }
                                    st.last_event_at = Some(now);
                                }
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    }

                    if paused_clone.load(Ordering::Relaxed) {
                        continue;
                    }

                    // Check debounced events and periodic intervals
                    let now = Instant::now();
                    let mut due_backups: Vec<(String, TriggerType)> = Vec::new();

                    for (pid, st) in watched.iter_mut() {
                        if suspended.contains(pid) {
                            continue;
                        }
                        let debounce = Duration::from_millis(st.project.debounce_ms);
                        let interval = Duration::from_secs(st.project.interval_secs);

                        if let Some(last_ev) = st.last_event_at {
                            if now.duration_since(last_ev) >= debounce {
                                due_backups.push((pid.clone(), TriggerType::Watcher));
                                st.dirty_since = None;
                                st.last_event_at = None;
                                st.last_backup_at = now;
                                continue;
                            }
                        }

                        if now.duration_since(st.last_backup_at) >= interval {
                            due_backups.push((pid.clone(), TriggerType::Auto));
                            st.last_backup_at = now;
                        }
                    }

                    for (pid, trigger) in due_backups {
                        on_event(MonitoringEvent {
                            kind: "status".to_string(),
                            project_id: Some(pid.clone()),
                            state: Some("backing_up".to_string()),
                            checkpoint_id: None,
                            message: None,
                        });

                        match engine.create_checkpoint(CreateCheckpointOptions {
                            project_id: pid.clone(),
                            label: None,
                            note: None,
                            trigger_type: trigger,
                            force_if_unchanged: false,
                        }) {
                            Ok(outcome) => {
                                if let Some(cp) = outcome.checkpoint {
                                    on_event(MonitoringEvent {
                                        kind: "checkpoint_created".to_string(),
                                        project_id: Some(pid.clone()),
                                        state: Some("monitoring".to_string()),
                                        checkpoint_id: Some(cp.id),
                                        message: Some(cp.label),
                                    });
                                } else {
                                    on_event(MonitoringEvent {
                                        kind: "status".to_string(),
                                        project_id: Some(pid.clone()),
                                        state: Some("monitoring".to_string()),
                                        checkpoint_id: None,
                                        message: None,
                                    });
                                }
                            }
                            Err(err) => {
                                on_event(MonitoringEvent {
                                    kind: "error".to_string(),
                                    project_id: Some(pid.clone()),
                                    state: Some("error".to_string()),
                                    checkpoint_id: None,
                                    message: Some(err.to_string()),
                                });
                            }
                        }
                    }
                }
            })
            .expect("Failed to spawn monitoring thread");

        Self {
            paused,
            tx: ctrl_tx,
            handle: Mutex::new(Some(handle)),
        }
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    pub fn set_paused(&self, paused: bool) {
        let _ = self.tx.send(ControlMsg::SetPaused(paused));
    }

    pub fn reload_projects(&self) {
        let _ = self.tx.send(ControlMsg::ReloadProjects);
    }

    pub fn suspend_project(&self, project_id: &str) {
        let _ = self
            .tx
            .send(ControlMsg::SuspendProject(project_id.to_string()));
    }

    pub fn resume_project(&self, project_id: &str) {
        let _ = self
            .tx
            .send(ControlMsg::ResumeProject(project_id.to_string()));
    }

    pub fn shutdown(&self) {
        let _ = self.tx.send(ControlMsg::Shutdown);
        if let Ok(mut guard) = self.handle.lock() {
            if let Some(h) = guard.take() {
                let _ = h.join();
            }
        }
    }
}

impl Drop for MonitorService {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn is_relevant_event_kind(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    )
}
