//! App-wide update status. Downloads run outside the editor; restart requires acknowledged saves.
use crate::FilmcraftApp;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::Child,
    time::{Duration, Instant},
};

pub struct State {
    pub open: bool,
    pub check_requested: bool,
    started: Instant,
    last_check: Option<Instant>,
    last_poll: Instant,
    child: Option<Child>,
    status: Value,
    restart: bool,
    ready_notified: bool,
    health: Option<PathBuf>,
    health_check: Option<std::sync::mpsc::Receiver<bool>>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            open: false,
            check_requested: false,
            started: Instant::now(),
            last_check: None,
            last_poll: Instant::now(),
            child: None,
            status: json!({"state":"idle","message":"Updates have not been checked yet."}),
            restart: false,
            ready_notified: false,
            health: None,
            health_check: None,
        }
    }
}
impl State {
    pub fn saving(&self) -> bool {
        self.restart || self.status["state"] == "closing"
    }
}
fn directory() -> PathBuf {
    if let Some(root) = std::env::var_os("YOUCRAFT_UI_CACHE_DIR") {
        return PathBuf::from(root).join("updates");
    }
    PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default()).join("YouCraft/updates")
}
fn script() -> Result<PathBuf, String> {
    if std::env::var_os("YOUCRAFT_BACKEND_URL").is_some() {
        return Err("Source development build: update the Git checkout and rebuild. Installed releases update automatically.".into());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let path = exe.parent().ok_or("Missing application folder")?.join("Background-Update.ps1");
    if !path.is_file() {
        return Err("Install the latest YouCraft release once to enable automatic updates.".into());
    }
    Ok(path)
}
fn launch(mode: &str) -> Result<Child, String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(script()?)
            .args(["-Mode", mode, "-EditorPid", &std::process::id().to_string()])
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = mode;
        Err("Automatic updates are available in the Windows installation.".into())
    }
}
fn error(s: &mut State, message: String) {
    s.status = json!({"state":"error","message":message});
    s.restart = false;
    s.open = true;
}
pub fn tick(app: &mut FilmcraftApp, ctx: &egui::Context) {
    let mut s = std::mem::take(&mut app.updates);
    if ctx.cumulative_frame_nr() > 3 && app.ui_error.is_none() && app.podcast.error.is_empty() {
        if let Some(ready) = s.health_check.as_ref().and_then(|rx| rx.try_recv().ok()) {
            s.health_check = None;
            if ready {
                if let Some(path) = s.health.take()
                    && let Err(e) = std::fs::write(path, "ready")
                {
                    error(&mut s, format!("Could not acknowledge update startup: {e}"));
                }
            } else {
                error(&mut s, "The analysis helper did not restart. The previous app backup is retained.".into());
            }
        }
    }
    let automatic =
        s.started.elapsed() > Duration::from_secs(10) && s.last_check.is_none_or(|t| t.elapsed() > Duration::from_secs(4 * 3600)) && script().is_ok();
    if (std::mem::take(&mut s.check_requested) || automatic) && s.child.is_none() && !s.restart {
        s.last_check = Some(Instant::now());
        match launch("Prepare") {
            Ok(child) => {
                s.child = Some(child);
                s.status = json!({"state":"checking","message":"Checking for updates…"});
            }
            Err(e) => error(&mut s, e),
        }
    }
    if s.last_poll.elapsed() >= Duration::from_secs(1) && s.child.is_some() {
        s.last_poll = Instant::now();
        if let Ok(text) = std::fs::read_to_string(directory().join("status.json"))
            && let Ok(value) = serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}'))
        {
            s.status = value;
        }
        if let Some(child) = s.child.as_mut() {
            match child.try_wait() {
                Ok(Some(exit)) => {
                    s.child = None;
                    if !exit.success() && s.status["state"] != "error" {
                        error(&mut s, "Update check failed. Retry when connected.".into());
                    }
                }
                Err(e) => {
                    s.child = None;
                    error(&mut s, e.to_string());
                }
                _ => {}
            }
        }
    }
    if (s.status["state"] == "ready" || s.status["state"] == "recovery") && !s.ready_notified {
        s.open = true;
        s.ready_notified = true;
    }
    if s.restart {
        match app.podcast.save_for_update() {
            Ok(false) => {}
            Ok(true) => match save_session(app, &directory()).and_then(|_| launch("Apply")) {
                Ok(child) => {
                    app.session.saved_revision = app.session.revision;
                    s.child = Some(child);
                    s.restart = false;
                    s.status = json!({"state":"closing","message":"Restarting to update…"});
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Err(e) => error(&mut s, e),
            },
            Err(e) => error(&mut s, format!("Restart cancelled: {e}")),
        }
    }
    if s.open {
        let mut open = true;
        egui::Window::new("YouCraft updates").open(&mut open).resizable(false).default_width(360.).show(ctx, |ui| {
            ui.label(s.status["message"].as_str().unwrap_or("Checking updates…"));
            if (s.status["state"] == "ready" || s.status["state"] == "recovery") && !s.restart && s.child.is_none() {
                ui.horizontal(|ui| {
                    let r = ui.button(if s.status["state"] == "recovery" { "Restart to restore" } else { "Restart to update" });
                    app.auto.add_response("updates.restart", &r, "Restart to update");
                    if r.clicked() {
                        app.podcast.error.clear();
                        s.restart = true;
                    }
                    let r = ui.button("Later");
                    app.auto.add_response("updates.later", &r, "Later");
                    if r.clicked() {
                        s.open = false;
                    }
                });
            } else if s.child.is_none() && !s.restart {
                let r = ui.button("Check again");
                app.auto.add_response("updates.check", &r, "Check again");
                if r.clicked() {
                    s.check_requested = true;
                    s.ready_notified = false;
                }
            }
            if s.restart {
                ui.label("Saving your project…");
            }
        });
        s.open &= open;
    }
    ctx.request_repaint_after(Duration::from_secs(1));
    app.updates = s;
}
fn save_session(app: &mut FilmcraftApp, root: &std::path::Path) -> Result<(), String> {
    if app.session.jobs.iter().any(|j| j.to_json()["finished"] != true)
        || !app.session.media_jobs.is_empty()
        || !app.session.mask_jobs.is_empty()
        || !app.session.scene_jobs.is_empty()
    {
        return Err("Finish native exports and background editing jobs before restarting.".into());
    }
    app.playback.playing = false;
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let original_path = app.session.path.clone();
    if original_path.is_some() {
        app.session.execute("file.save", json!({})).map_err(|e| e.to_string())?;
    }
    let snapshot = root.join("resume.fcproj");
    app.session.execute("file.saveCopy", json!({"path":snapshot})).map_err(|e| e.to_string())?;
    let resume = json!({"native":snapshot,"original_path":original_path,"podcast":app.podcast,
        "projects_open":app.podcast.projects_open,"ui":app.ui,"workspace":app.workspaces.current,
        "playhead":app.session.playhead().seconds()});
    let data = serde_json::to_vec(&resume).map_err(|e| e.to_string())?;
    write_resume(&root.join("resume.json"), &data)?;
    Ok(())
}
/// Save a restart snapshot without closing. Also used by the authenticated MCP/control path.
pub fn checkpoint(app: &mut FilmcraftApp) -> Result<Value, String> {
    if !app.podcast.save_for_update()? {
        return Ok(json!({"ready":false}));
    }
    save_session(app, &directory())?;
    Ok(json!({"ready":true,"resume":directory().join("resume.json")}))
}
fn write_resume(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes).and_then(|_| std::fs::rename(tmp, path)).map_err(|e| e.to_string())
}
pub fn restore(app: &mut FilmcraftApp) -> Result<(), String> {
    let Some(resume) = std::env::var_os("YOUCRAFT_UPDATE_RESUME") else {
        return Ok(());
    };
    let value: Value = serde_json::from_slice(&std::fs::read(resume).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    restore_value(app, value)?;
    app.updates.health = std::env::var_os("YOUCRAFT_UPDATE_HEALTH").map(PathBuf::from);
    if app.updates.health.is_some() {
        let endpoint = app.podcast.endpoint.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        app.updates.health_check = Some(rx);
        std::thread::spawn(move || {
            let ready = std::panic::catch_unwind(|| {
                for _ in 0..30 {
                    if crate::podcast::helper_available(&endpoint) {
                        return true;
                    }
                    std::thread::sleep(Duration::from_secs(1));
                }
                false
            })
            .unwrap_or(false);
            let _ = tx.send(ready);
        });
    }
    Ok(())
}
fn restore_value(app: &mut FilmcraftApp, value: Value) -> Result<(), String> {
    app.session.execute("file.open", json!({"path":value["native"]})).map_err(|e| e.to_string())?;
    app.session.path = value["original_path"].as_str().map(str::to_owned);
    let mut podcast: crate::podcast::State = serde_json::from_value(value["podcast"].clone()).map_err(|e| e.to_string())?;
    podcast.token = app.podcast.token.clone();
    podcast.endpoint = app.podcast.endpoint.clone();
    podcast.projects_open = value["projects_open"].as_bool().unwrap_or(true);
    podcast.restored_after_update();
    app.podcast = podcast;
    app.ui = serde_json::from_value(value["ui"].clone()).map_err(|e| e.to_string())?;
    if let Some(workspace) = value["workspace"].as_str() {
        // UiState already contains the exact docking/floating layout. Selecting the preset
        // again would overwrite those unsaved layout adjustments.
        app.workspaces.current = workspace.into();
    }
    if app.session.active_sequence().is_some() {
        app.session.execute("playhead.set", json!({"seconds":value["playhead"]})).map_err(|e| e.to_string())?;
    }
    app.playback.playing = false;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (FilmcraftApp, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "youcraft-resume-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let mut app = FilmcraftApp::new(filmcraft_engine::Session::default());
        app.session.execute("file.newSequence", json!({"name":"Update fixture","fps":25})).unwrap();
        app.set_workspace("Podcast");
        app.podcast.projects_open = false;
        app.podcast.step = 5;
        app.podcast.token = "must-not-be-written".into();
        (app, root)
    }
    #[test]
    fn restart_preserves_native_edit_page_and_playhead_without_credentials() {
        let (mut app, root) = fixture();
        app.session.execute("playhead.set", json!({"seconds":12.0})).unwrap();
        save_session(&mut app, &root).unwrap();
        let bytes = std::fs::read(root.join("resume.json")).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("must-not-be-written"));
        let mut reopened = FilmcraftApp::new(filmcraft_engine::Session::default());
        restore_value(&mut reopened, serde_json::from_slice(&bytes).unwrap()).unwrap();
        assert!(reopened.session.active_sequence().is_some());
        assert_eq!(reopened.session.playhead(), app.session.playhead());
        assert_eq!(reopened.podcast.step, 5);
        assert!(!reopened.podcast.projects_open);
        assert!(!reopened.playback.playing);
        assert_eq!(reopened.ui.workspace, "Podcast");
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn save_failure_does_not_mark_native_edit_saved() {
        let (mut app, root) = fixture();
        let before = app.session.saved_revision;
        std::fs::write(&root, "a file cannot be a save directory").unwrap();
        assert!(save_session(&mut app, &root).is_err());
        assert_eq!(app.session.saved_revision, before);
        assert!(app.session.is_dirty());
        std::fs::remove_file(root).unwrap();
    }
    #[test]
    fn active_analysis_and_save_errors_block_restart() {
        let mut state = crate::podcast::State::default();
        state.job = Some("analysis-running".into());
        assert!(state.save_for_update().is_err());
        state.job = None;
        state.error = "Disk full".into();
        assert!(state.save_for_update().is_err());
    }
}
