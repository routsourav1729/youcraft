//! Podcast workspace inside Filmcraft. HTTP/model work never runs on the paint thread.
use crate::{FilmcraftApp, dock::PanelKind, state::Mode};
use egui::{Rect, Response};
use serde_json::{Value, json};
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};

type Reply = Result<Value, String>;
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct State {
    pub endpoint: String,
    #[serde(skip)]
    pub token: String,
    pub remote: bool,
    pub compute_device: String,
    pub project_id: String,
    pub name: String,
    pub project: Value,
    pub step: usize,
    pub review_page: usize,
    pub activity_page: usize,
    pub edit_page: usize,
    pub master: String,
    pub camera_a: Vec<String>,
    pub camera_b: Vec<String>,
    pub browser_path: String,
    pub browser: Vec<Value>,
    pub destination: usize,
    pub samples: Vec<Value>,
    pub job: Option<String>,
    pub job_kind: String,
    pub status: String,
    pub error: String,
    pub roman: String,
    pub replacement: String,
    pub spelling_choices: Vec<String>,
    pub target_word: String,
    pub target_word_index: Option<usize>,
    pub caption_id: String,
    pub references: std::collections::BTreeMap<String, f64>,
    pub tolerance: f64,
    pub interval: f64,
    pub spacing: f64,
    pub cache: String,
    pub assembled: bool,
    pub output_job: String,
    pub text_drafts: std::collections::BTreeMap<String, String>,
    #[serde(skip)]
    pending: Option<Receiver<Reply>>,
    #[serde(skip)]
    action: String,
    #[serde(skip)]
    last_poll: Option<Instant>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            endpoint: "http://127.0.0.1:8765".into(),
            token: String::new(),
            remote: false,
            compute_device: "auto".into(),
            project_id: String::new(),
            name: "YouCraft episode".into(),
            project: Value::Null,
            step: 1,
            review_page: 0,
            activity_page: 0,
            edit_page: 0,
            master: String::new(),
            camera_a: vec![],
            camera_b: vec![],
            browser_path: String::new(),
            browser: vec![],
            destination: 0,
            samples: vec![],
            job: None,
            job_kind: String::new(),
            status: String::new(),
            error: String::new(),
            roman: String::new(),
            replacement: String::new(),
            spelling_choices: Vec::new(),
            target_word: String::new(),
            target_word_index: None,
            caption_id: String::new(),
            references: Default::default(),
            tolerance: 10.,
            interval: 60.,
            spacing: 15.,
            cache: std::env::var("LOCALAPPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|_| std::env::temp_dir())
                .join("YouCraft")
                .join("media")
                .to_string_lossy()
                .into(),
            assembled: false,
            output_job: String::new(),
            text_drafts: Default::default(),
            pending: None,
            action: String::new(),
            last_poll: None,
        }
    }
}
#[derive(Clone)]
struct Client {
    http: reqwest::blocking::Client,
    base: String,
    token: String,
}
impl Client {
    fn new(s: &State) -> Result<Self, String> {
        let url = reqwest::Url::parse(&s.endpoint).map_err(|e| e.to_string())?;
        if url.scheme() != "http" || !matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]")) {
            return Err("Use a local backend or an SSH tunnel at http://127.0.0.1:8765".into());
        }
        Ok(Self {
            http: reqwest::blocking::Client::builder()
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(7200))
                .build()
                .map_err(|e| e.to_string())?,
            base: s.endpoint.trim_end_matches('/').into(),
            token: s.token.clone(),
        })
    }
    fn request(&self, method: reqwest::Method, path: &str, body: Option<&Value>) -> Reply {
        let mut req = self.http.request(method, format!("{}{path}", self.base)).bearer_auth(&self.token).timeout(Duration::from_secs(60));
        if let Some(body) = body {
            req = req.json(body);
        }
        let response = req.send().map_err(|e| e.to_string())?;
        let status = response.status();
        let mut text = String::new();
        response.take(32 * 1024 * 1024 + 1).read_to_string(&mut text).map_err(|e| e.to_string())?;
        if text.len() > 32 * 1024 * 1024 {
            return Err("Backend response exceeds 32 MB".into());
        }
        if !status.is_success() {
            return Err(format!("{status}: {text}"));
        }
        serde_json::from_str(&text).map_err(|e| e.to_string())
    }
    fn get(&self, path: &str) -> Reply {
        self.request(reqwest::Method::GET, path, None)
    }
    fn post(&self, path: &str, body: Value) -> Reply {
        self.request(reqwest::Method::POST, path, Some(&body))
    }
    fn download(&self, path: &str, destination: &Path) -> Result<(), String> {
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut response =
            self.http.get(format!("{}{path}", self.base)).bearer_auth(&self.token).send().and_then(|r| r.error_for_status()).map_err(|e| e.to_string())?;
        let partial = destination.with_extension("partial");
        let result = (|| {
            let mut file = std::fs::File::create(&partial).map_err(|e| e.to_string())?;
            response.copy_to(&mut file).map_err(|e| e.to_string())?;
            std::fs::rename(&partial, destination).map_err(|e| e.to_string())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(partial);
        }
        result
    }
}
impl State {
    fn busy(&self) -> bool {
        self.pending.is_some() || self.job.is_some()
    }
    fn start(&mut self, action: &str, work: impl FnOnce(Client) -> Reply + Send + 'static) {
        if self.pending.is_some() {
            return;
        }
        let client = match Client::new(self) {
            Ok(c) => c,
            Err(e) => {
                self.error = e;
                return;
            }
        };
        self.error.clear();
        self.action = action.into();
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(client))).unwrap_or_else(|_| Err("Background request failed".into()));
            let _ = tx.send(result);
        });
    }
    fn refresh(&mut self) {
        let id = self.project_id.clone();
        self.start("project", move |c| c.get(&format!("/v1/projects/{id}")));
    }
    fn save(&mut self) {
        let p = self.project.clone();
        let id = self.project_id.clone();
        self.start("project", move |c| c.request(reqwest::Method::PUT, &format!("/v1/projects/{id}"), Some(&p)));
    }
    fn job(&mut self, kind: &str, options: Value) {
        let id = self.project_id.clone();
        let kind = kind.to_string();
        self.job_kind = kind.clone();
        self.project["settings"]["compute_device"] = json!(self.compute_device);
        let project = self.project.clone();
        let save_first = matches!(kind.as_str(), "diarize" | "sync" | "transcribe" | "shots" | "reactions" | "proxies" | "attach_cameras");
        self.start("job", move |c| {
            let saved = if save_first { c.request(reqwest::Method::PUT, &format!("/v1/projects/{id}"), Some(&project))? } else { Value::Null };
            // Preserve the saved revision even if the job submission fails.
            match c.post(&format!("/v1/projects/{id}/jobs"), json!({"kind":kind,"options":options})) {
                Ok(job) => Ok(json!({"job":job,"project":saved})),
                Err(error) => Ok(json!({"error":error,"project":saved})),
            }
        });
    }
    fn add(&mut self, path: String) {
        match self.destination {
            0 => self.master = path,
            1 => self.camera_a.push(path),
            _ => self.camera_b.push(path),
        }
    }
}
fn string(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").into()
}
fn number(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(0.)
}
fn rows(v: &Value, key: &str) -> Vec<Value> {
    v.get(key).and_then(Value::as_array).cloned().unwrap_or_default()
}
fn button(app: &mut FilmcraftApp, ui: &mut egui::Ui, id: &str, label: &str) -> bool {
    let r = ui.button(label);
    app.auto.add_response(&format!("podcast.{id}"), &r, label);
    r.clicked()
}
fn field(app: &mut FilmcraftApp, ui: &mut egui::Ui, id: &str, value: &mut String) -> Response {
    let r = ui.add(egui::TextEdit::singleline(value).desired_width(ui.available_width().min(360.)));
    app.auto.add_response(&format!("podcast.{id}"), &r, id);
    r
}
fn numeric(app: &mut FilmcraftApp, ui: &mut egui::Ui, value: &mut Value, key: &str, id: &str) {
    let mut n = number(value, key);
    let r = ui.add(egui::DragValue::new(&mut n).speed(0.05));
    app.auto.add_response(&format!("podcast.{id}.{key}"), &r, key);
    if r.changed() {
        value[key] = json!(n);
    }
}
fn set_rows(s: &mut State, key: &str, list: Vec<Value>) {
    s.project[key] = json!(list);
}

fn validate_project(project: &Value) -> Result<(), String> {
    if project.is_null() {
        return Ok(());
    }
    if !project.is_object() || project.get("settings").is_some_and(|v| !v.is_null() && !v.is_object()) {
        return Err("Invalid workflow project/settings. Reopen the episode from the backend.".into());
    }
    for key in ["clips", "activity", "captions", "reactions", "shots"] {
        if let Some(value) = project.get(key)
            && !value.is_null()
            && !value.as_array().is_some_and(|rows| rows.iter().all(Value::is_object))
        {
            return Err(format!("Invalid workflow {key}. Reopen the episode from the backend."));
        }
    }
    Ok(())
}

fn decode_state(bytes: &[u8]) -> Result<State, String> {
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("Workflow state exceeds 8 MB".into());
    }
    let state: State = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    validate_project(&state.project)?;
    Ok(state)
}

fn load_state(path: &Path) -> Result<State, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path).and_then(|file| file.take(8 * 1024 * 1024 + 1).read_to_end(&mut bytes)).map_err(|e| e.to_string())?;
    decode_state(&bytes)
}
fn sample(s: &mut State, start: f64, end: f64) {
    s.job("sample", json!({"start":start,"end":end}));
}
fn poll(app: &mut FilmcraftApp, s: &mut State) {
    let result = s.pending.as_ref().and_then(|rx| match rx.try_recv() {
        Ok(v) => Some(v),
        Err(mpsc::TryRecvError::Disconnected) => Some(Err("Worker disconnected".into())),
        Err(_) => None,
    });
    if let Some(result) = result {
        s.pending = None;
        match result {
            Err(e) => s.error = e,
            Ok(value) => match s.action.as_str() {
                "project" => {
                    s.project_id = string(&value, "id");
                    s.compute_device = value["settings"]["compute_device"].as_str().unwrap_or("auto").into();
                    s.project = value;
                    s.status = "Saved".into();
                }
                "created" => {
                    s.project_id = string(&value, "id");
                    s.project = value;
                    let master = s.master.clone();
                    s.job("import", json!({"master":master,"A":[],"B":[]}));
                }
                "projects" => {
                    s.status = value
                        .as_array()
                        .map(|a| a.iter().map(|p| format!("{}: {}", string(p, "name"), string(p, "id"))).collect::<Vec<_>>().join("\n"))
                        .unwrap_or_default();
                }
                "capabilities" => {
                    let count = value["configured_providers"].as_object().map_or(0, |v| v.len());
                    s.status = format!("Connected · {count} models configured");
                }
                "list" => {
                    s.browser = value.as_array().cloned().unwrap_or_default();
                }
                "uploaded" => {
                    for p in value.as_array().into_iter().flatten() {
                        if let Some(p) = p.as_str() {
                            s.add(p.into());
                        }
                    }
                }
                "samples" => {
                    s.samples = value.as_array().cloned().unwrap_or_default();
                }
                "job" => {
                    if value["project"].is_object() {
                        s.project = value["project"].clone();
                    }
                    if let Some(error) = value["error"].as_str() {
                        s.error = error.into();
                    } else {
                        s.job = Some(string(&value["job"], "id"));
                        s.status = format!("{} queued", s.job_kind);
                    }
                }
                "cancel" => {
                    s.status = "Cancelling…".into();
                }
                "poll" => {
                    let state = string(&value, "state");
                    s.status = format!("{} · {} · {:.0}%", s.job_kind, state, number(&value, "progress") * 100.);
                    if ["succeeded", "failed", "cancelled", "interrupted"].contains(&state.as_str()) {
                        let jid = s.job.take().unwrap_or_default();
                        if state == "succeeded" {
                            if matches!(s.job_kind.as_str(), "sample" | "reaction_preview" | "reference_frame") {
                                let filename = match s.job_kind.as_str() {
                                    "sample" => "voice-sample.wav",
                                    "reference_frame" => "reference.png",
                                    _ => "reaction.mp4",
                                };
                                let dest = PathBuf::from(&s.cache).join(format!("{jid}-{filename}"));
                                s.start("play", move |c| {
                                    c.download(&format!("/v1/jobs/{jid}/files/{filename}"), &dest)?;
                                    Ok(json!({"path":dest}))
                                });
                            } else if s.job_kind == "filmcraft_export" {
                                s.output_job = jid;
                                s.status = "Export ready".into();
                            } else if s.job_kind == "transliterate" {
                                s.replacement = string(&value["result"], "text");
                                s.spelling_choices = value["result"]["candidates"]
                                    .as_array()
                                    .map(|v| v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
                                    .unwrap_or_default();
                            } else if s.job_kind == "proxies" {
                                let cache = PathBuf::from(&s.cache);
                                let pid = s.project_id.clone();
                                s.start("assemble", move |c| {
                                    let mut plan = c.get(&format!("/v1/jobs/{jid}/files/filmcraft.json"))?;
                                    for asset in plan.get_mut("assets").and_then(Value::as_array_mut).ok_or("Missing assets")? {
                                        let proxy = string(asset, "proxy");
                                        if proxy.is_empty() || Path::new(&proxy).file_name().and_then(|n| n.to_str()) != Some(proxy.as_str()) {
                                            return Err("Invalid proxy filename".into());
                                        }
                                        let dest = cache.join(&pid).join(&proxy);
                                        if !dest.is_file() {
                                            c.download(&format!("/v1/projects/{pid}/proxies/{proxy}"), &dest)?;
                                        }
                                        asset["path"] = json!(dest);
                                    }
                                    Ok(plan)
                                });
                            } else {
                                if s.job_kind == "attach_cameras" {
                                    s.camera_a.clear();
                                    s.camera_b.clear();
                                }
                                s.refresh();
                            }
                        } else {
                            s.error = string(&value, "error");
                        }
                    }
                }
                "play" => {
                    let result = app.session.execute("file.import", json!({"paths":[value["path"]]}));
                    match result.and_then(|v| {
                        let item = v["items"]
                            .as_array()
                            .and_then(|a| a.first())
                            .and_then(Value::as_u64)
                            .ok_or_else(|| filmcraft_engine::EngineError::Other("Cannot load preview".into()))?;
                        app.session.execute("source.open", json!({"item":item}))
                    }) {
                        Ok(_) => {
                            app.show_panel(PanelKind::Source);
                            app.source_playback.clock.looping = s.job_kind == "reaction_preview";
                            if s.job_kind != "reference_frame"
                                && let Err(e) = app.play_source()
                            {
                                s.error = e;
                            }
                        }
                        Err(e) => s.error = e.to_string(),
                    }
                }
                "downloaded" => {
                    s.status = format!("Saved to {}", string(&value, "folder"));
                }
                "assemble" => {
                    if value["project"].is_object() {
                        s.project = value["project"].clone();
                    }
                    if let Some(error) = value["error"].as_str() {
                        s.error = error.into();
                    } else {
                        match app.session.execute("podcast.assemble", value.get("plan").cloned().unwrap_or(value)) {
                            Ok(_) => {
                                s.assembled = true;
                                s.step = 6;
                                app.ui.mode = Mode::Edit;
                                app.ui.text_tab = "Transcript".into();
                                s.status = "Edit ready".into();
                            }
                            Err(e) => s.error = e.to_string(),
                        }
                    }
                }
                _ => {}
            },
        }
    }
    if s.pending.is_none() && s.job.is_some() && s.last_poll.is_none_or(|t| t.elapsed() > Duration::from_secs(1)) {
        let id = s.job.clone().unwrap_or_default();
        s.last_poll = Some(Instant::now());
        s.start("poll", move |c| c.get(&format!("/v1/jobs/{id}")));
    }
}

/// Local MCP uses the same state and asynchronous jobs as the visible workspace.
pub fn control(app: &mut FilmcraftApp, method: &str, p: &Value) -> Reply {
    let s = &mut app.podcast;
    if method == "podcast.inspect" {
        return Ok(json!({"busy":s.busy(),"project_id":s.project_id,"project":s.project,
            "master":s.master,"camera_a":s.camera_a,"camera_b":s.camera_b,
            "compute_device":s.compute_device,"samples":s.samples,"job":s.job,
            "job_kind":s.job_kind,"status":s.status,"error":s.error,"assembled":s.assembled,
            "actions":["create","refresh","samples","save","lock_roles","attach_cameras","diarize","sync","transcribe","shots","reactions","sample","reference_frame","reaction_preview","transliterate","open_edit","cancel"]}));
    }
    if s.busy() && !(method == "podcast.action" && p["action"] == "cancel") {
        return Err("Podcast is busy; poll podcast.inspect or cancel the job".into());
    }
    match method {
        "podcast.configure" => {
            let fields = p.as_object().ok_or("Expected an object")?;
            let mut state = serde_json::to_value(&*s).map_err(|e| e.to_string())?;
            for (key, value) in fields {
                if !["name", "master", "camera_a", "camera_b", "compute_device", "project_id"].contains(&key.as_str()) {
                    return Err(format!("Unsupported setting: {key}"));
                }
                if matches!(key.as_str(), "camera_a" | "camera_b") {
                    let paths = value.as_array().ok_or("Camera paths must be arrays")?;
                    if paths.len() > 256 || paths.iter().any(|v| v.as_str().is_none_or(|s| s.is_empty() || s.len() > 32768)) {
                        return Err("Invalid camera paths (maximum 256)".into());
                    }
                } else if value.as_str().is_none_or(|s| s.len() > 32768) {
                    return Err("Expected a bounded string".into());
                }
                state[key] = value.clone();
            }
            if !matches!(state["compute_device"].as_str(), Some("auto" | "cpu" | "cuda")) {
                return Err("compute_device must be auto, cpu or cuda".into());
            }
            let mut updated: State = serde_json::from_value(state).map_err(|e| e.to_string())?;
            updated.token = s.token.clone();
            updated.error.clear();
            *s = updated;
        }
        "podcast.review" => {
            if !s.project.is_object() {
                return Err("Load a project first".into());
            }
            let mut project = s.project.clone();
            for (key, value) in p.as_object().ok_or("Expected an object")? {
                if !["settings", "activity", "captions", "clips", "reactions", "shots"].contains(&key.as_str()) {
                    return Err(format!("Unsupported review field: {key}"));
                }
                project[key] = value.clone();
            }
            if serde_json::to_vec(&project).map_err(|e| e.to_string())?.len() > 8 * 1024 * 1024 {
                return Err("Review exceeds 8 MB".into());
            }
            validate_project(&project)?;
            // Backend validates times, IDs and relationships before committing the revision.
            let id = s.project_id.clone();
            s.start("project", move |c| c.request(reqwest::Method::PUT, &format!("/v1/projects/{id}"), Some(&project)));
        }
        "podcast.action" => {
            let action = p["action"].as_str().ok_or("Missing action")?;
            let options = p.get("options").cloned().unwrap_or(json!({}));
            if !options.is_object() {
                return Err("options must be an object".into());
            }
            if !matches!(action, "create" | "cancel") && s.project_id.is_empty() {
                return Err("Create or load a project first".into());
            }
            match action {
                "create" => {
                    if s.master.is_empty() {
                        return Err("Choose master audio first".into());
                    }
                    let name = s.name.clone();
                    s.start("created", move |c| c.post("/v1/projects", json!({"name":name})));
                }
                "refresh" => s.refresh(),
                "save" => s.save(),
                "samples" => {
                    let id = s.project_id.clone();
                    s.start("samples", move |c| c.get(&format!("/v1/projects/{id}/speaker-samples")));
                }
                "lock_roles" => {
                    lock_roles(s, p["host"].as_str().ok_or("Missing host speaker ID")?)?;
                    s.save();
                }
                "attach_cameras" => s.job(action, json!({"A":s.camera_a,"B":s.camera_b})),
                "diarize" | "sync" | "transcribe" | "shots" | "reactions" | "sample" | "reference_frame" | "reaction_preview" | "transliterate" => {
                    s.job(action, options)
                }
                "open_edit" => {
                    let id = s.project_id.clone();
                    let project = s.project.clone();
                    s.start("assemble", move |c| {
                        let saved = c.request(reqwest::Method::PUT, &format!("/v1/projects/{id}"), Some(&project))?;
                        match c.get(&format!("/v1/projects/{id}/filmcraft")) {
                            Ok(plan) => Ok(json!({"plan":plan,"project":saved})),
                            Err(error) => Ok(json!({"error":error,"project":saved})),
                        }
                    });
                }
                "cancel" => {
                    if s.pending.is_some() {
                        return Err("Request in progress; retry cancel after polling".into());
                    }
                    let id = s.job.clone().ok_or("No active job")?;
                    s.start("cancel", move |c| c.post(&format!("/v1/jobs/{id}/cancel"), json!({})));
                }
                _ => return Err(format!("Unknown podcast action: {action}")),
            }
        }
        _ => return Err("Unknown podcast method".into()),
    }
    Ok(json!({"queued":s.pending.is_some(),"busy":s.busy(),"error":s.error}))
}

fn lock_roles(s: &mut State, host: &str) -> Result<(), String> {
    let speakers: std::collections::BTreeSet<_> = rows(&s.project, "activity").iter().map(|a| string(a, "speaker")).filter(|s| !s.is_empty()).collect();
    if !speakers.contains(host) {
        return Err("Choose an identified host speaker".into());
    }
    let mapping: serde_json::Map<_, _> = speakers
        .into_iter()
        .map(|speaker| {
            let camera = if speaker == host { "A" } else { "B" };
            (speaker, json!(camera))
        })
        .collect();
    s.project["settings"]["host_speaker"] = json!(host);
    s.project["settings"]["speaker_camera"] = json!(mapping);
    s.project["settings"]["roles_locked"] = json!(true);
    Ok(())
}

pub fn tick(app: &mut FilmcraftApp, ctx: &egui::Context) {
    let mut s = std::mem::take(&mut app.podcast);
    if s.token.is_empty()
        && !s.remote
        && let Ok(root) = std::env::var("LOCALAPPDATA")
        && let Ok(token) = std::fs::read_to_string(PathBuf::from(root).join("Betun/api-token"))
    {
        s.token = token.trim().into();
    }
    poll(app, &mut s);
    if s.busy() {
        ctx.request_repaint_after(Duration::from_millis(100));
    }
    app.podcast = s;
}

pub fn show(app: &mut FilmcraftApp, ui: &mut egui::Ui, rect: Rect) {
    let mut s = std::mem::take(&mut app.podcast);
    if let Err(error) = validate_project(&s.project) {
        s.error = error;
        s.step = 0;
    }
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink(12.)).layout(egui::Layout::top_down(egui::Align::Min)));
    let ui = &mut child;
    ui.heading("YouCraft");
    ui.horizontal_wrapped(|ui| {
        for (i, label) in [(1, "Audio"), (2, "Speakers"), (3, "Cameras"), (4, "Transcript"), (5, "Reactions"), (6, "Export"), (0, "Settings")] {
            let r = ui.selectable_label(s.step == i, label);
            app.auto.add(&format!("podcast.step.{i}"), r.rect, label);
            if r.clicked() {
                s.step = i;
            }
        }
    });
    ui.separator();
    if !s.status.is_empty() {
        ui.label(&s.status);
    }
    if !s.error.is_empty() {
        ui.colored_label(egui::Color32::LIGHT_RED, &s.error);
    }
    if s.busy() {
        ui.spinner();
        if s.job.is_some() && s.pending.is_none() && button(app, ui, "cancel", "Cancel") {
            let id = s.job.clone().unwrap_or_default();
            s.start("cancel", move |c| c.post(&format!("/v1/jobs/{id}/cancel"), json!({})));
        }
        ui.ctx().request_repaint_after(Duration::from_millis(100));
    }
    egui::ScrollArea::vertical().id_salt("podcast-body").show(ui, |ui| {
        ui.add_enabled_ui(!s.busy(), |ui| match s.step {
            0 => connect(app, ui, &mut s),
            1 => audio(app, ui, &mut s),
            2 => speakers(app, ui, &mut s),
            3 => cameras(app, ui, &mut s),
            4 => review(app, ui, &mut s),
            5 => reactions(app, ui, &mut s),
            _ => editing(app, ui, &mut s),
        });
    });
    app.podcast = s;
}
fn connect(app: &mut FilmcraftApp, ui: &mut egui::Ui, s: &mut State) {
    if button(app, ui, "connection_test", "Check setup") {
        s.start("capabilities", move |c| c.get("/v1/capabilities"));
    }
    ui.label("Episode ID");
    field(app, ui, "project_id", &mut s.project_id);
    if button(app, ui, "resume", "Open project") {
        s.refresh();
    }
    if button(app, ui, "list_projects", "Saved episodes") {
        s.start("projects", move |c| c.get("/v1/projects"));
    }
    if button(app, ui, "save_connection", "Save workflow") {
        let path = PathBuf::from(&s.cache).join("workflow.json");
        let result =
            std::fs::create_dir_all(&s.cache).map_err(|e| e.to_string()).and_then(|_| serde_json::to_vec_pretty(&s).map_err(|e| e.to_string())).and_then(|b| {
                if b.len() > 8 * 1024 * 1024 {
                    return Err("Workflow state exceeds 8 MB; resume this episode by project ID instead".into());
                }
                std::fs::write(path, b).map_err(|e| e.to_string())
            });
        if let Err(e) = result {
            s.error = e;
        } else {
            s.status = "Workflow saved".into();
        }
    }
    if button(app, ui, "restore_connection", "Restore workflow") {
        match load_state(&PathBuf::from(&s.cache).join("workflow.json")) {
            Ok(mut restored) => {
                restored.token = s.token.clone();
                *s = restored;
            }
            Err(e) => s.error = e,
        }
    }
    ui.collapsing("Advanced", |ui| {
        ui.label("Address");
        field(app, ui, "endpoint", &mut s.endpoint);
        ui.label("Access token");
        let r = ui.add(egui::TextEdit::singleline(&mut s.token).password(true));
        app.auto.add("podcast.token", r.rect, "Access token");
        let r = ui.checkbox(&mut s.remote, "Remote testing");
        app.auto.add_response("podcast.remote", &r, "Remote processing");
        ui.label("Preview cache");
        field(app, ui, "cache", &mut s.cache);
    });
}
fn picker(app: &mut FilmcraftApp, ui: &mut egui::Ui, s: &mut State) {
    if s.step == 1 {
        s.destination = 0;
    } else {
        if s.destination == 0 {
            s.destination = 1;
        }
        ui.horizontal(|ui| {
            for (i, label) in [(1, "Host camera"), (2, "Guest camera")] {
                let r = ui.selectable_value(&mut s.destination, i, label);
                app.auto.add_response(&format!("podcast.destination.{i}"), &r, label);
            }
        });
    }
    if button(app, ui, "files", "Add files…") {
        let paths = app.hooks.pick_files.as_mut().map(|f| f(&["wav", "mp3", "m4a", "mp4", "mov", "mkv"])).unwrap_or_default();
        if s.remote && !paths.is_empty() {
            s.start("uploaded", move |c| {
                let mut result = vec![];
                for path in paths {
                    let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
                    let name = Path::new(&path).file_name().and_then(|n| n.to_str()).ok_or("Invalid filename")?;
                    let mut url = reqwest::Url::parse(&format!("{}/v1/uploads/", c.base)).map_err(|e| e.to_string())?;
                    url.path_segments_mut().map_err(|_| "Invalid URL")?.pop_if_empty().push(name);
                    let response = c
                        .http
                        .put(url)
                        .bearer_auth(&c.token)
                        .body(file)
                        .send()
                        .and_then(|r| r.error_for_status())
                        .map_err(|e| e.to_string())?
                        .json::<Value>()
                        .map_err(|e| e.to_string())?;
                    result.push(response["path"].clone());
                }
                Ok(json!(result))
            });
        } else {
            for p in paths {
                s.add(p);
            }
        }
    }
    if s.remote {
        ui.label("Server folder (relative to configured media root)");
        field(app, ui, "server_folder", &mut s.browser_path);
        if button(app, ui, "browse", "Browse server media") {
            let path = s.browser_path.clone();
            s.start("list", move |c| {
                let mut url = reqwest::Url::parse(&format!("{}/v1/server-media", c.base)).map_err(|e| e.to_string())?;
                url.query_pairs_mut().append_pair("path", &path);
                c.get(&format!("{}?{}", url.path(), url.query().unwrap_or("")))
            });
        }
        for row in s.browser.clone() {
            if button(
                app,
                ui,
                &format!("media.{}", string(&row, "relative")),
                &format!("{} {}", if row["directory"] == true { "Folder" } else { "Add" }, string(&row, "name")),
            ) {
                if row["directory"] == true {
                    s.browser_path = string(&row, "relative");
                } else {
                    s.add(string(&row, "path"));
                }
            }
        }
    }
}
fn audio(app: &mut FilmcraftApp, ui: &mut egui::Ui, s: &mut State) {
    ui.label("Episode");
    field(app, ui, "name", &mut s.name);
    ui.label("Master audio");
    field(app, ui, "master", &mut s.master);
    picker(app, ui, s);
    ui.horizontal(|ui| {
        ui.label("Process on");
        for (device, label) in [("auto", "Auto"), ("cpu", "CPU"), ("cuda", "GPU")] {
            let r = ui.selectable_value(&mut s.compute_device, device.into(), label);
            app.auto.add_response(&format!("podcast.device.{device}"), &r, label);
        }
    });
    if button(app, ui, "create", "Import") {
        let name = s.name.clone();
        s.start("created", move |c| c.post("/v1/projects", json!({"name":name})));
    }
    if !s.project_id.is_empty() && button(app, ui, "diarize", "Identify speakers") {
        s.job("diarize", json!({}));
    }
}
fn speakers(app: &mut FilmcraftApp, ui: &mut egui::Ui, s: &mut State) {
    if button(app, ui, "samples", "Speaker samples") {
        let id = s.project_id.clone();
        s.start("samples", move |c| c.get(&format!("/v1/projects/{id}/speaker-samples")));
    }
    for row in s.samples.clone() {
        let speaker = string(&row, "speaker");
        ui.group(|ui| {
            ui.strong(&speaker);
            if button(app, ui, &format!("listen.{speaker}"), "Listen") {
                sample(s, number(&row, "start"), number(&row, "end"));
            }
            let host = s.project["settings"]["host_speaker"].as_str() == Some(speaker.as_str());
            let r = ui.radio(host, "Host");
            app.auto.add_response(&format!("podcast.host.{speaker}"), &r, "Choose host");
            if r.clicked() {
                s.project["settings"]["host_speaker"] = json!(speaker);
            }
        });
    }
    if button(app, ui, "lock_roles", "Confirm speakers") {
        let host = string(&s.project["settings"], "host_speaker");
        match lock_roles(s, &host) {
            Ok(()) => s.save(),
            Err(error) => s.error = error,
        }
    }
}
fn shard_list(app: &mut FilmcraftApp, ui: &mut egui::Ui, paths: &mut Vec<String>, id: &str) {
    let mut action = None;
    for (i, p) in paths.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(format!("{} · {}", i + 1, Path::new(p).file_name().unwrap_or_default().to_string_lossy()));
            if button(app, ui, &format!("{id}.up.{i}"), "↑") && i > 0 {
                action = Some((i, 0));
            }
            if button(app, ui, &format!("{id}.down.{i}"), "↓") && i + 1 < paths.len() {
                action = Some((i, 1));
            }
            if button(app, ui, &format!("{id}.remove.{i}"), "Remove") {
                action = Some((i, 2));
            }
        });
    }
    if let Some((i, a)) = action {
        match a {
            0 => paths.swap(i, i - 1),
            1 => paths.swap(i, i + 1),
            _ => {
                paths.remove(i);
            }
        }
    }
}
fn cameras(app: &mut FilmcraftApp, ui: &mut egui::Ui, s: &mut State) {
    picker(app, ui, s);
    ui.strong("A · Host");
    shard_list(app, ui, &mut s.camera_a, "a");
    ui.strong("B · Guest");
    shard_list(app, ui, &mut s.camera_b, "b");
    if button(app, ui, "attach", "Add recordings") {
        s.job("attach_cameras", json!({"A":s.camera_a,"B":s.camera_b}));
    }
    if button(app, ui, "sync", "Sync") {
        s.job("sync", json!({}));
    }
    let mut clips = rows(&s.project, "clips");
    for clip in &mut clips {
        if clip["camera"] == "master" {
            continue;
        }
        let sync_id = format!("sync.{}", string(clip, "id"));
        ui.group(|ui| {
            ui.label(format!("{} · {}", string(clip, "camera"), string(clip, "path")));
            ui.label(format!("Confidence: {:?} · {}", clip["sync_score"], clip["warnings"]));
            ui.horizontal(|ui| {
                ui.label("Offset (s)");
                numeric(app, ui, clip, "offset", &sync_id);
                ui.label("Clock drift");
                numeric(app, ui, clip, "rate", &sync_id);
            });
            let mut reviewed = clip["sync_reviewed"] == true;
            let r = ui.checkbox(&mut reviewed, "Sync reviewed");
            app.auto.add_response(&format!("podcast.sync.reviewed.{}", string(clip, "id")), &r, "Sync reviewed");
            if r.changed() {
                clip["sync_reviewed"] = json!(reviewed);
            }
        });
    }
    set_rows(s, "clips", clips);
    if button(app, ui, "save_sync", "Save sync") {
        s.save();
    }
    if button(app, ui, "transcribe", "Transcribe") {
        s.job("transcribe", json!({}));
    }
}
fn review(app: &mut FilmcraftApp, ui: &mut egui::Ui, s: &mut State) {
    ui.collapsing("Speaker segments", |ui| {
        let mut activity = rows(&s.project, "activity");
        let mut remove = None;
        pager(app, ui, &mut s.activity_page, activity.len(), "activity");
        for (i, a) in activity.iter_mut().enumerate().skip(s.activity_page * 40).take(40) {
            ui.push_id(i, |ui| {
                ui.horizontal_wrapped(|ui| {
                    numeric(app, ui, a, "start", &format!("activity.{i}"));
                    numeric(app, ui, a, "end", &format!("activity.{i}"));
                    let mut speaker = string(a, "speaker");
                    let r = ui.add(egui::TextEdit::singleline(&mut speaker).desired_width(110.));
                    app.auto.add_response(&format!("podcast.activity.speaker.{i}"), &r, "Speaker");
                    a["speaker"] = json!(speaker);
                    if button(app, ui, &format!("activity.remove.{i}"), "Remove") {
                        remove = Some(i);
                    }
                });
            });
        }
        if let Some(i) = remove {
            activity.remove(i);
        }
        set_rows(s, "activity", activity);
    });
    let mut captions = rows(&s.project, "captions");
    if button(app, ui, "remove_empty_captions", "Remove blank text") {
        captions.retain(|c| !string(c, "text").trim().is_empty());
    }
    let mut remove = None;
    pager(app, ui, &mut s.review_page, captions.len(), "captions");
    for (i, c) in captions.iter_mut().enumerate().skip(s.review_page * 40).take(40) {
        ui.push_id(i, |ui| {
            ui.group(|ui| {
                let mut speaker = string(c, "speaker");
                if field(app, ui, &format!("caption.speaker.{i}"), &mut speaker).changed() {
                    c["speaker"] = json!(speaker);
                    c["reviewed"] = json!(false);
                }
                ui.horizontal_wrapped(|ui| {
                    numeric(app, ui, c, "start", &format!("caption.{i}"));
                    numeric(app, ui, c, "end", &format!("caption.{i}"));
                    if button(app, ui, &format!("caption.play.{i}"), "Listen") {
                        sample(s, number(c, "start"), number(c, "end"));
                    }
                    if button(app, ui, &format!("caption.remove.{i}"), "Remove text") {
                        remove = Some(i);
                    }
                });
                let mut text = string(c, "text");
                let r = ui.add(egui::TextEdit::multiline(&mut text).desired_width(ui.available_width()));
                app.auto.add_response(&format!("podcast.caption.text.{i}"), &r, "Odia text");
                if r.changed() {
                    c["text"] = json!(text);
                    c["reviewed"] = json!(false);
                }
                if c["overlap"] == true {
                    ui.label("Overlapping speech");
                }
                if button(app, ui, &format!("caption.roman.{i}"), "Romanized Odia") {
                    s.caption_id = string(c, "id");
                    s.target_word_index = None;
                    s.target_word.clear();
                }
                if s.caption_id == string(c, "id") {
                    ui.horizontal_wrapped(|ui| {
                        for (word_index, word) in string(c, "text").split_whitespace().enumerate() {
                            if button(app, ui, &format!("word.{i}.{word_index}"), word) {
                                s.target_word = word.into();
                                s.target_word_index = Some(word_index);
                            }
                        }
                    });
                }
                let mut reviewed = c["reviewed"] == true;
                let r = ui.checkbox(&mut reviewed, "Reviewed");
                app.auto.add_response(&format!("podcast.caption.reviewed.{i}"), &r, "Reviewed");
                if r.changed() {
                    c["reviewed"] = json!(reviewed);
                }
            });
        });
    }
    if let Some(i) = remove {
        captions.remove(i);
    }
    if !s.caption_id.is_empty() {
        ui.separator();
        ui.label("Selected word");
        if field(app, ui, "target_word", &mut s.target_word).changed() {
            s.target_word_index = None;
        }
        ui.label("Romanized spelling");
        field(app, ui, "roman", &mut s.roman);
        if button(app, ui, "transliterate", "Suggest") {
            s.job("transliterate", json!({"text":s.roman}));
        }
        for choice in s.spelling_choices.clone() {
            if button(app, ui, &format!("spelling.{choice}"), &choice) {
                s.replacement = choice;
            }
        }
        ui.label("Odia spelling");
        field(app, ui, "replacement", &mut s.replacement);
        if button(app, ui, "replace_word", "Replace")
            && let Some(c) = captions.iter_mut().find(|c| string(c, "id") == s.caption_id)
        {
            let text = string(c, "text");
            if !s.target_word.is_empty() && text.contains(&s.target_word) && !s.replacement.is_empty() {
                match replace_word(&text, s.target_word_index, &s.target_word, &s.replacement) {
                    Ok(text) => {
                        c["text"] = json!(text);
                        c["reviewed"] = json!(false);
                    }
                    Err(error) => s.error = error,
                }
            } else {
                s.error = "Select an existing word and a nonempty replacement".into();
            }
        }
    }
    set_rows(s, "captions", captions);
    if button(app, ui, "save_review", "Save corrections") {
        s.save();
    }
    if button(app, ui, "base_shots", "Create cuts") {
        s.job("shots", json!({}));
    }
}

fn replace_word(text: &str, selected: Option<usize>, expected: &str, replacement: &str) -> Result<String, String> {
    let mut cursor = 0;
    for (index, word) in text.split_whitespace().enumerate() {
        let start = text.get(cursor..).and_then(|rest| rest.find(word)).map(|offset| cursor + offset).ok_or("Select the word again")?;
        let end = start + word.len();
        if selected.map_or(word == expected, |selected| selected == index) {
            if word != expected {
                return Err("The text changed; select the word again".into());
            }
            let mut corrected = text.to_owned();
            corrected.replace_range(start..end, replacement);
            return Ok(corrected);
        }
        cursor = end;
    }
    Err("Select an existing word".into())
}

fn reactions(app: &mut FilmcraftApp, ui: &mut egui::Ui, s: &mut State) {
    let host = string(&s.project["settings"], "host_speaker");
    let camera = s.project["settings"]["speaker_camera"][&host].clone();
    for clip in rows(&s.project, "clips") {
        if clip["camera"] == camera {
            let id = string(&clip, "id");
            ui.label(string(&clip, "path"));
            let reference = s.references.entry(id.clone()).or_default();
            ui.horizontal(|ui| {
                ui.label("Reference (s)");
                let r = ui.add(egui::DragValue::new(reference).range(0.0..=number(&clip, "duration")));
                app.auto.add_response(&format!("podcast.reference.time.{id}"), &r, "Reference time");
            });
            let time = *reference;
            if button(app, ui, &format!("reference.{id}"), "View reference") {
                s.job("reference_frame", json!({"clip_id":id,"time":time}));
            }
        }
    }
    let r = ui.add(egui::Slider::new(&mut s.tolerance, 3.0..=30.0).text("Gaze tolerance °"));
    app.auto.add_response("podcast.gaze_tolerance", &r, "Gaze tolerance");
    let r = ui.add(egui::Slider::new(&mut s.interval, 15.0..=180.0).text("Interval (s)"));
    app.auto.add_response("podcast.reaction_interval", &r, "Review interval");
    let r = ui.add(egui::Slider::new(&mut s.spacing, 5.0..=60.0).text("Spacing (s)"));
    app.auto.add_response("podcast.reaction_spacing", &r, "Minimum spacing");
    if button(app, ui, "analyze_reactions", "Find reactions") {
        s.job("reactions", json!({"references":s.references,"tolerance":s.tolerance,"interval":s.interval,"spacing":s.spacing}));
    }
    let mut reactions = rows(&s.project, "reactions");
    let base_seconds: f64 = rows(&s.project, "shots")
        .iter()
        .filter(|shot| shot["layout"] == camera || shot["layout"] == "both")
        .map(|shot| number(shot, "end") - number(shot, "start"))
        .sum();
    let added_seconds: f64 = reactions.iter().filter(|r| r["decision"] == "keep").map(|r| number(r, "end") - number(r, "start")).sum();
    ui.label(format!("Host: {:.1} min · Reactions: +{:.1} min", base_seconds.max(0.) / 60., added_seconds.max(0.) / 60.));
    if button(app, ui, "reject_all", "Skip pending") {
        for r in &mut reactions {
            if r["decision"] == "pending" {
                r["decision"] = json!("reject");
            }
        }
    }
    let mut previous_interval = None;
    for (i, r) in reactions.iter_mut().enumerate() {
        let interval = (number(r, "start") / s.interval.max(1.)).floor() as u64;
        if previous_interval != Some(interval) {
            ui.separator();
            ui.strong(format!("{:.0}–{:.0}s", interval as f64 * s.interval, interval.saturating_add(1) as f64 * s.interval));
            previous_interval = Some(interval);
        }
        ui.group(|ui| {
            let cues = r["cues"].as_array().map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" · ")).unwrap_or_default();
            ui.label(format!("{:.1}–{:.1}s · {cues}", number(r, "start"), number(r, "end")));
            if button(app, ui, &format!("reaction.preview.{i}"), "Preview") {
                s.job("reaction_preview", json!({"reaction_id":r["id"]}));
            }
            ui.horizontal(|ui| {
                for (decision, label) in [("pending", "Review"), ("keep", "Keep"), ("reject", "Reject")] {
                    let response = ui.selectable_label(r["decision"] == decision, label);
                    app.auto.add_response(&format!("podcast.reaction.{i}.{decision}"), &response, decision);
                    if response.clicked() {
                        r["decision"] = json!(decision);
                    }
                }
            });
            app.auto.add(&format!("podcast.reaction.{i}"), ui.min_rect(), "Reaction suggestion");
        });
    }
    set_rows(s, "reactions", reactions);
    if button(app, ui, "save_reactions", "Save choices") {
        s.save();
    }
    if s.remote && button(app, ui, "open_editor", "Open edit") {
        s.job("proxies", json!({}));
    }
    if !s.remote && button(app, ui, "open_originals", "Open edit") {
        let id = s.project_id.clone();
        let project = s.project.clone();
        s.start("assemble", move |c| {
            let saved = c.request(reqwest::Method::PUT, &format!("/v1/projects/{id}"), Some(&project))?;
            match c.get(&format!("/v1/projects/{id}/filmcraft")) {
                Ok(plan) => Ok(json!({"plan":plan,"project":saved})),
                Err(error) => Ok(json!({"error":error,"project":saved})),
            }
        });
    }
}
fn editing(app: &mut FilmcraftApp, ui: &mut egui::Ui, s: &mut State) {
    ui.weak("Estimated word timing").on_hover_text("Check playback before cutting individual words.");
    if button(app, ui, "text_panel", "Transcript") {
        app.show_panel(PanelKind::Text);
    }
    let words = filmcraft_engine::transcript::sequence_words(&app.session);
    ui.collapsing("Edit words", |ui| {
        pager(app, ui, &mut s.edit_page, words.len(), "edit");
        for (i, w) in words.iter().enumerate().skip(s.edit_page * 40).take(40) {
            ui.push_id(i, |ui| {
                let key = format!("{}:{}", w.item.0, w.index);
                let text = s.text_drafts.entry(key.clone()).or_insert_with(|| w.text.clone());
                field(app, ui, &format!("edit.word.{i}"), text);
                let replacement = text.clone();
                if button(app, ui, &format!("edit.apply.{i}"), "Apply") {
                    match app.session.execute("podcast.correct", json!({"item":w.item.0,"index":w.index,"text":replacement})) {
                        Err(e) => s.error = e.to_string(),
                        Ok(_) => {
                            s.text_drafts.remove(&key);
                        }
                    }
                }
                if button(app, ui, &format!("edit.reset.{i}"), "Reset") {
                    s.text_drafts.remove(&key);
                }
            });
        }
    });
    if button(app, ui, "export_srt", "Export subtitles…") {
        let path = app.hooks.pick_save.as_mut().and_then(|f| f("episode.or.srt"));
        if let Some(path) = path {
            match app.session.execute("podcast.exportSrt", json!({"path":path})) {
                Ok(_) => s.status = "Edited UTF-8 SRT saved".into(),
                Err(e) => s.error = e.to_string(),
            }
        }
    }
    if button(app, ui, "export_video", "Export video…") {
        app.ui.mode = Mode::Export;
    }
    if s.remote
        && button(app, ui, "export_originals", "Render originals")
        && let Some(sequence) = app.session.state.active_sequence
    {
        match serde_json::to_value(&*app.session.project) {
            Ok(snapshot) => s.job("filmcraft_export", json!({"snapshot":snapshot,"sequence":sequence.0})),
            Err(e) => s.error = e.to_string(),
        }
    }
    if !s.output_job.is_empty()
        && button(app, ui, "download_export", "Download export…")
        && let Some(folder) = app.hooks.pick_folder.as_mut().and_then(|f| f())
    {
        let jid = s.output_job.clone();
        s.start("downloaded", move |c| {
            for filename in ["episode.mp4", "episode.or.srt"] {
                c.download(&format!("/v1/jobs/{jid}/files/{filename}"), &PathBuf::from(&folder).join(filename))?;
            }
            Ok(json!({"folder":folder}))
        });
    }
}

fn pager(app: &mut FilmcraftApp, ui: &mut egui::Ui, page: &mut usize, count: usize, id: &str) {
    let last = count.saturating_sub(1) / 40;
    *page = (*page).min(last);
    ui.horizontal(|ui| {
        let r = ui.add_enabled(*page > 0, egui::Button::new("Previous"));
        app.auto.add_response(&format!("podcast.{id}.previous"), &r, "Previous page");
        if r.clicked() {
            *page = page.saturating_sub(1);
        }
        ui.label(format!("Page {} / {}", *page + 1, last + 1));
        let r = ui.add_enabled(*page < last, egui::Button::new("Next"));
        app.auto.add_response(&format!("podcast.{id}.next"), &r, "Next page");
        if r.clicked() {
            *page += 1;
        }
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn corrupt_workflow_rows_are_rejected_before_the_editor_mutates_them() {
        for value in [serde_json::json!("bad"), serde_json::json!({"settings":12}), serde_json::json!({"captions":[12]})] {
            let bytes = serde_json::to_vec(&serde_json::json!({"project":value})).unwrap();
            assert!(super::decode_state(&bytes).is_err());
        }
        assert!(super::decode_state(br#"{"project":{"captions":[],"settings":{}}}"#).is_ok());
        assert!(super::decode_state(&vec![b' '; 8 * 1024 * 1024 + 1]).is_err());
    }
    #[test]
    fn roman_correction_targets_the_selected_unicode_occurrence() {
        assert_eq!(super::replace_word("ବାର  ବାର", Some(1), "ବାର", "ଥର").unwrap(), "ବାର  ଥର");
        assert!(super::replace_word("ବାର ବାର", Some(1), "ବଦଳ", "ଥର").is_err());
    }
}
