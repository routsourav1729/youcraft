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
    #[serde(skip)]
    pub setup: crate::podcast_setup::State,
    #[serde(skip)]
    pub ai_setup: crate::podcast_setup::State,
    #[serde(skip)]
    pub audio_output: crate::podcast_audio::State,
    #[serde(skip)]
    pub projects_open: bool,
    #[serde(skip)]
    project_home: String,
    pub endpoint: String,
    #[serde(skip)]
    pub token: String,
    pub remote: bool,
    pub compute_device: String,
    pub monitor_volume_db: f64,
    pub monitor_muted: bool,
    pub focus_video: bool,
    pub video_fraction: f32,
    pub camera_video_fraction: f32,
    pub transcript_video_fraction: f32,
    #[serde(skip)]
    diagnostics_text: Option<String>,
    #[serde(skip)]
    reaction_undo: Option<(String, Value, Value)>,
    #[serde(skip)]
    reference_candidates: Value,
    #[serde(skip)]
    reference_candidate_clip: String,
    #[serde(skip)]
    reference_thumbnails: std::collections::BTreeMap<String, egui::TextureHandle>,
    pub reference_image: String,
    pub show_detection: bool,
    pub preview_suggestions: bool,
    #[serde(skip)]
    recalibrating: bool,
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
    #[serde(skip)]
    auto_identify: bool,
    #[serde(skip)]
    follow_up: String,
    #[serde(skip)]
    saved_project: Value,
    #[serde(skip)]
    edited_at: Option<Instant>,
    #[serde(skip)]
    episodes: Vec<Value>,
    #[serde(skip)]
    review_item: Option<u64>,
    #[serde(skip)]
    review_active: bool,
    #[serde(skip)]
    review_seek: f64,
    #[serde(skip)]
    review_clip: Option<String>,
    #[serde(skip)]
    review_autoplay: bool,
    #[serde(skip)]
    review_is_edit: bool,
    #[serde(skip)]
    review_reaction: Option<String>,
    #[serde(skip)]
    storage: Value,
    #[serde(skip)]
    confirm_delete: bool,
    #[serde(skip)]
    startup: Option<Instant>,
    #[serde(skip)]
    pending_regeneration: Option<(String, Value)>,
    #[serde(skip)]
    retry_job: Option<(String, Value)>,
    #[serde(skip)]
    refresh_capabilities: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            setup: Default::default(),
            ai_setup: Default::default(),
            audio_output: Default::default(),
            projects_open: true,
            project_home: String::new(),
            endpoint: "http://127.0.0.1:8765".into(),
            monitor_volume_db: 0.,
            monitor_muted: false,
            focus_video: false,
            video_fraction: 0.64,
            camera_video_fraction: 0.46,
            transcript_video_fraction: 0.64,
            diagnostics_text: None,
            reaction_undo: None,
            reference_candidates: Value::Null,
            reference_candidate_clip: String::new(),
            reference_thumbnails: Default::default(),
            reference_image: String::new(),
            show_detection: true,
            preview_suggestions: true,
            recalibrating: false,
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
            cache: std::env::var("YOUCRAFT_UI_CACHE_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| std::env::var("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir()).join("YouCraft").join("media"))
                .to_string_lossy()
                .into(),
            assembled: false,
            output_job: String::new(),
            text_drafts: Default::default(),
            pending: None,
            action: String::new(),
            last_poll: None,
            auto_identify: false,
            follow_up: String::new(),
            saved_project: Value::Null,
            edited_at: None,
            episodes: Vec::new(),
            review_item: None,
            review_active: false,
            review_seek: 0.,
            review_clip: None,
            review_autoplay: false,
            review_is_edit: false,
            review_reaction: None,
            storage: Value::Null,
            confirm_delete: false,
            startup: None,
            pending_regeneration: None,
            retry_job: None,
            refresh_capabilities: false,
        }
    }
}
const WRONG_SERVICE: &str = "Wrong local service. Restart YouCraft.";

fn validate_endpoint(endpoint: &str) -> Result<(), String> {
    let url = reqwest::Url::parse(endpoint).map_err(|e| e.to_string())?;
    if url.scheme() != "http" || !matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]")) {
        return Err("Use a local helper address at http://127.0.0.1".into());
    }
    if !matches!(url.path(), "" | "/") || url.query().is_some() || url.fragment().is_some() || !url.username().is_empty() || url.password().is_some() {
        return Err("Use the helper address without a file or folder path".into());
    }
    Ok(())
}

#[derive(PartialEq, Eq)]
enum Service {
    Ready,
    Starting,
    Other,
}

fn probe_service(http: &reqwest::blocking::Client, endpoint: &str) -> Service {
    let response = match http.get(format!("{}/health", endpoint.trim_end_matches('/'))).timeout(Duration::from_millis(500)).send() {
        Ok(response) => response,
        Err(_) => return Service::Starting,
    };
    if !response.status().is_success() {
        return Service::Other;
    }
    let mut body = String::new();
    if response.take(8193).read_to_string(&mut body).is_err() {
        return Service::Starting;
    }
    if body.len() > 8192 {
        return Service::Other;
    }
    let Ok(value) = serde_json::from_str::<Value>(&body) else { return Service::Other };
    let identity = value["service"] == "youcraft" && value["protocol_version"] == 1;
    // Older helpers lack the setup contract; start the installed helper beside them.
    if value["status"] == "ok" && value["schema_version"] == 1 && identity { Service::Ready } else { Service::Other }
}

pub fn helper_available(endpoint: &str) -> bool {
    if validate_endpoint(endpoint).is_err() {
        return false;
    }
    reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_millis(150))
        .build()
        .is_ok_and(|http| probe_service(&http, endpoint) == Service::Ready)
}

pub fn local_token() -> String {
    let root = std::env::var_os("BETUN_DATA_DIR").map(PathBuf::from).or_else(|| std::env::var_os("LOCALAPPDATA").map(|root| PathBuf::from(root).join("Betun")));
    root.and_then(|root| std::fs::read_to_string(root.join("api-token")).ok()).unwrap_or_default().trim().into()
}

fn response_value(status: reqwest::StatusCode, text: &str) -> Reply {
    if status == reqwest::StatusCode::NOT_IMPLEMENTED || text.trim_start().starts_with('<') {
        return Err(WRONG_SERVICE.into());
    }
    let value: Value = serde_json::from_str(text).map_err(|_| WRONG_SERVICE.to_string())?;
    if !status.is_success() {
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err("Local session expired. Restart YouCraft.".into());
        }
        let detail = value.get("detail").unwrap_or(&value);
        let error = detail.as_str().map(str::to_string).unwrap_or_else(|| detail.to_string());
        return Err(error.chars().take(1200).collect());
    }
    Ok(value)
}

fn display_error(error: String) -> String {
    if error.contains("No diarize provider configured") || error.contains("No transcribe provider configured") {
        "Speech models missing. Open Model setup.".into()
    } else {
        error
    }
}

#[derive(Clone)]
struct Client {
    http: reqwest::blocking::Client,
    base: String,
    token: String,
    local: bool,
}
impl Client {
    fn new(s: &State) -> Result<Self, String> {
        validate_endpoint(&s.endpoint)?;
        Ok(Self {
            http: reqwest::blocking::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(7200))
                .build()
                .map_err(|e| e.to_string())?,
            base: s.endpoint.trim_end_matches('/').into(),
            token: s.token.clone(),
            local: !s.remote,
        })
    }
    fn wait_ready(&self) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(12);
        loop {
            match probe_service(&self.http, &self.base) {
                Service::Ready => break,
                Service::Other => return Err(WRONG_SERVICE.into()),
                Service::Starting if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(100)),
                Service::Starting => return Err("Local helper did not start. Restart YouCraft; check analysis-helper.log if it persists.".into()),
            }
        }
        Ok(())
    }
    fn request(&self, method: reqwest::Method, path: &str, body: Option<&Value>) -> Reply {
        if method != reqwest::Method::GET {
            self.wait_ready()?;
        }
        let token = if self.local && self.token.is_empty() { local_token() } else { self.token.clone() };
        let timeout = if path.contains("review-plan") || path.ends_with("/filmcraft") { 900 } else { 60 };
        let mut req = self.http.request(method, format!("{}{path}", self.base)).bearer_auth(token).timeout(Duration::from_secs(timeout));
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
        response_value(status, &text)
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
    pub(crate) fn save_for_update(&mut self) -> Result<bool, String> {
        if self.job.is_some() || self.setup.busy() || self.ai_setup.busy() {
            return Err("Finish analysis or setup before restarting to update.".into());
        }
        if self.pending.is_some() {
            return Ok(false);
        }
        if !self.error.is_empty() {
            return Err(self.error.clone());
        }
        if !self.project_id.is_empty() && self.project != self.saved_project {
            self.save();
            return Ok(false);
        }
        Ok(true)
    }
    pub(crate) fn restart_view(&self) -> Value {
        json!({"item":self.review_item,"active":self.review_active,"edit":self.review_is_edit,
            "clip":self.review_clip,"reaction":self.review_reaction,"capabilities":self.setup.capabilities})
    }
    pub(crate) fn restored_after_update(&mut self, view: &Value, active_sequence: Option<u64>) {
        self.saved_project = self.project.clone();
        self.setup.check_on_start = false;
        self.setup.checked = true;
        self.setup.capabilities = view["capabilities"].clone();
        self.refresh_capabilities = true;
        self.review_item = view["item"].as_u64();
        self.review_active = view["active"] == true && active_sequence.is_some() && self.review_item == active_sequence;
        self.review_is_edit = view["edit"] == true;
        self.review_clip = view["clip"].as_str().map(str::to_owned);
        self.review_reaction = view["reaction"].as_str().map(str::to_owned);
    }
    fn busy(&self) -> bool {
        self.pending.is_some() || self.job.is_some() || self.setup.busy()
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
    pub fn checkpoint_on_exit(&self) {
        if self.project_id.is_empty() || self.project == self.saved_project {
            return;
        }
        if self.project_id.len() != 32 || !self.project_id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return;
        }
        let path = PathBuf::from(&self.cache).join(format!("unsaved-{}.json", self.project_id));
        let temporary = path.with_extension("tmp");
        let result = std::fs::create_dir_all(&self.cache)
            .map_err(|error| error.to_string())
            .and_then(|_| serde_json::to_vec(self).map_err(|error| error.to_string()))
            .and_then(|bytes| std::fs::write(&temporary, bytes).and_then(|_| std::fs::rename(&temporary, &path)).map_err(|error| error.to_string()));
        if let Err(error) = result {
            log::error!("Could not preserve podcast corrections: {error}");
        }
    }
    fn review_preview(&mut self, edit: bool) {
        self.review_clip = None;
        self.review_reaction = None;
        self.review_autoplay = false;
        self.request_review(edit, None);
    }
    fn request_review(&mut self, edit: bool, clip_id: Option<String>) {
        let framing = edit && self.step == 3;
        let include_pending = edit && self.preview_suggestions;
        let reaction = self.review_reaction.clone();
        let project = self.project.clone();
        let id = self.project_id.clone();
        self.start("review_assemble", move |c| {
            let saved = c.request(reqwest::Method::PUT, &format!("/v1/projects/{id}"), Some(&project))?;
            let mut query = format!("/v1/projects/{id}/review-plan?edit={edit}&include_pending={include_pending}&framing={framing}");
            if let Some(clip) = clip_id {
                if !clip.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
                    return Err("Invalid camera shard identifier".into());
                }
                query.push_str(&format!("&clip_id={clip}"));
            }
            if let Some(reaction) = reaction {
                if !reaction.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
                    return Err("Invalid reaction identifier".into());
                }
                query.push_str(&format!("&reaction_id={reaction}"));
            }
            match c.get(&query) {
                Ok(plan) => Ok(json!({"project":saved,"plan":plan})),
                Err(error) => Ok(json!({"project":saved,"error":error})),
            }
        });
    }
    fn list_projects(&mut self) {
        self.start("projects", |c| {
            c.wait_ready()?;
            Ok(json!({"projects":c.get("/v1/projects")?,"storage":c.get("/v1/storage")?}))
        });
    }
    fn reset_episode(&mut self) {
        self.checkpoint_on_exit();
        *self = State {
            endpoint: self.endpoint.clone(),
            token: self.token.clone(),
            cache: self.cache.clone(),
            setup: std::mem::take(&mut self.setup),
            ai_setup: std::mem::take(&mut self.ai_setup),
            audio_output: std::mem::take(&mut self.audio_output),
            project_home: self.project_home.clone(),
            projects_open: false,
            step: 1,
            ..Default::default()
        };
    }
    fn remember_view(&self) {
        if self.project_id.len() == 32 && self.project_id.bytes().all(|b| b.is_ascii_hexdigit()) {
            let _ = std::fs::create_dir_all(&self.cache);
            let _ = std::fs::write(PathBuf::from(&self.cache).join(format!("view-{}.json", self.project_id)), self.step.to_string());
        }
    }
    fn resume_step(&self) -> usize {
        let saved =
            std::fs::read_to_string(PathBuf::from(&self.cache).join(format!("view-{}.json", self.project_id))).ok().and_then(|v| v.parse::<usize>().ok());
        saved
            .unwrap_or_else(|| {
                if !rows(&self.project, "shots").is_empty() {
                    5
                } else if !rows(&self.project, "captions").is_empty() {
                    4
                } else if self.project["settings"]["roles_locked"] == true {
                    3
                } else if !rows(&self.project, "activity").is_empty() {
                    2
                } else {
                    1
                }
            })
            .clamp(1, 6)
    }

    fn remember_episode(&mut self) {
        let path = PathBuf::from(&self.cache).join("last-episode.txt");
        if let Err(error) = std::fs::create_dir_all(&self.cache).and_then(|_| std::fs::write(path, &self.project_id)) {
            self.error = format!("Episode saved, but could not remember it: {error}");
        }
    }
    fn refresh(&mut self) {
        let id = self.project_id.clone();
        self.start("project", move |c| c.get(&format!("/v1/projects/{id}")));
    }
    fn export_metadata(&mut self, path: String) {
        let project = self.project.clone();
        self.start("downloaded", move |_| {
            let bytes = serde_json::to_vec_pretty(&project).map_err(|e| e.to_string())?;
            if bytes.len() > 8 * 1024 * 1024 {
                return Err("Metadata exceeds 8 MB".into());
            }
            std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
            Ok(json!({"folder":path}))
        });
    }
    fn import_metadata(&mut self, path: String) {
        if self.assembled {
            self.error = "Open the workflow project before importing corrections.".into();
            return;
        }
        let id = self.project_id.clone();
        self.follow_up = if self.review_active { "review" } else { "" }.into();
        self.start("project", move |c| {
            let mut bytes = Vec::new();
            std::fs::File::open(path).and_then(|f| f.take(8 * 1024 * 1024 + 1).read_to_end(&mut bytes)).map_err(|e| e.to_string())?;
            if bytes.len() > 8 * 1024 * 1024 {
                return Err("Metadata exceeds 8 MB".into());
            }
            let project: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            validate_project(&project)?;
            c.request(reqwest::Method::PUT, &format!("/v1/projects/{id}"), Some(&project))
        });
    }
    pub(crate) fn native_saved_as(&mut self, path: &str) {
        if self.project_id.is_empty() || !self.project.is_object() {
            return;
        }
        if let Some(name) = Path::new(path).file_stem().and_then(|v| v.to_str()).filter(|v| !v.trim().is_empty()) {
            self.name = name.chars().take(200).collect();
            self.project["name"] = json!(self.name);
            if !self.project["reaction_calibration"].is_object() {
                self.project["reaction_calibration"] = json!({});
            }
            self.project["reaction_calibration"]["native_project"] = json!(path);
            self.project["reaction_calibration"]["native_ready"] = json!(self.assembled);
            self.edited_at = Some(Instant::now());
            self.checkpoint_on_exit();
        }
    }
    fn save(&mut self) {
        self.checkpoint_on_exit();
        let p = self.project.clone();
        let id = self.project_id.clone();
        self.start("project", move |c| c.request(reqwest::Method::PUT, &format!("/v1/projects/{id}"), Some(&p)));
    }
    fn job(&mut self, kind: &str, options: Value) {
        let output = match kind {
            "diarize" => "activity",
            "transcribe" => "captions",
            "shots" => "shots",
            "reactions" => "reactions",
            _ => "",
        };
        let existing_sync = kind == "sync" && rows(&self.project, "clips").iter().any(|c| c["sync_reviewed"] == true || c["sync_score"].is_number());
        if existing_sync || (!output.is_empty() && !rows(&self.project, output).is_empty()) {
            self.pending_regeneration = Some((kind.into(), options));
            return;
        }
        self.run_job(kind, options);
    }
    fn run_job(&mut self, kind: &str, options: Value) {
        self.checkpoint_on_exit();
        self.retry_job = Some((kind.into(), options.clone()));
        let id = self.project_id.clone();
        let kind = kind.to_string();
        self.job_kind = kind.clone();
        self.project["settings"]["compute_device"] = json!(self.compute_device);
        let project = self.project.clone();
        let save_first = matches!(
            kind.as_str(),
            "import"
                | "diarize"
                | "sync"
                | "transcribe"
                | "shots"
                | "reactions"
                | "reaction_select"
                | "proxies"
                | "attach_cameras"
                | "sync_preview"
                | "reference_suggestions"
                | "framing"
        );
        self.start("job", move |c| {
            let saved = if save_first { c.request(reqwest::Method::PUT, &format!("/v1/projects/{id}"), Some(&project))? } else { Value::Null };
            // Preserve the saved revision even if the job submission fails.
            match c.post(&format!("/v1/projects/{id}/jobs"), json!({"kind":kind,"options":options})) {
                Ok(job) => Ok(json!({"job":job,"project":saved})),
                Err(error) => Ok(json!({"error":error,"project":saved})),
            }
        });
    }
    fn import_audio(&mut self, identify: bool) {
        if self.master.trim().is_empty() {
            self.error = "Choose master audio first".into();
            return;
        }
        if self.setup.checked
            && !self.remote
            && (self.setup.capabilities["media"]["ffmpeg"] != true
                || self.setup.capabilities["media"]["ffprobe"] != true
                || (identify && self.setup.capabilities["installed_providers"]["diarize"] != true))
        {
            self.step = 0;
            self.error = "Install the speaker model and media tools first".into();
            return;
        }
        self.auto_identify = identify;
        self.follow_up.clear();
        self.samples.clear();
        self.status = "Importing audio…".into();
        let name = self.name.clone();
        self.start("created", move |c| c.post("/v1/projects", json!({"name":name})));
    }
    fn load_samples(&mut self) {
        let id = self.project_id.clone();
        self.start("samples", move |c| c.get(&format!("/v1/projects/{id}/speaker-samples")));
    }
    pub fn add_files(&mut self, paths: Vec<String>) {
        if self.busy() {
            self.error = "Wait for the current task".into();
            return;
        }
        if self.step == 1 {
            if paths.len() != 1 {
                self.error = "Choose one master audio file".into();
                return;
            }
            self.destination = 0;
        } else if self.step == 3 {
            if self.destination == 0 {
                self.destination = 1;
            }
        } else {
            self.error = "Open Audio or Cameras to add files".into();
            return;
        }
        for path in paths {
            self.add(path);
        }
        self.error.clear();
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
fn job_label(kind: &str) -> &str {
    match kind {
        "import" => "Importing audio",
        "diarize" => "Identifying speakers",
        "transcribe" => "Transcribing",
        _ => kind,
    }
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
            Err(e) => {
                s.error = display_error(e);
                if s.action == "project" && s.project != s.saved_project {
                    s.edited_at = Some(Instant::now());
                    s.checkpoint_on_exit();
                }
                s.auto_identify = false;
                s.follow_up.clear();
            }
            Ok(value) => match s.action.as_str() {
                "diagnostics" => {
                    s.diagnostics_text = Some(value.to_string());
                    s.status = "Model diagnostics copied. Review before pasting into a GitHub issue; nothing was uploaded.".into();
                }
                "project" => {
                    s.project_id = string(&value, "id");
                    s.compute_device = value["settings"]["compute_device"].as_str().unwrap_or("auto").into();
                    s.project = value;
                    if let Some(calibration) = s.project["reaction_calibration"].as_object() {
                        if let Some(refs) = calibration.get("references").and_then(Value::as_object) {
                            s.references = refs.iter().filter_map(|(id, t)| t.as_f64().map(|t| (id.clone(), t))).collect();
                        }
                        s.reference_image = calibration.get("reference_image").and_then(Value::as_str).unwrap_or("").into();
                        s.tolerance = calibration.get("tolerance").and_then(Value::as_f64).unwrap_or(s.tolerance);
                        s.spacing = calibration.get("spacing").and_then(Value::as_f64).unwrap_or(s.spacing);
                    }
                    if !s.recalibrating
                        && let Some(analysis) = s.project["reaction_analysis"].as_object().filter(|a| !a.is_empty())
                    {
                        if let Some(refs) = analysis.get("references").and_then(Value::as_object) {
                            s.references = refs.iter().filter_map(|(id, t)| t.as_f64().map(|t| (id.clone(), t))).collect();
                        }
                        s.interval = analysis.get("interval").and_then(Value::as_f64).unwrap_or(s.interval);
                        s.reference_image = analysis.get("reference_image").and_then(Value::as_str).unwrap_or("").into();
                    }
                    if let Some(master) = rows(&s.project, "clips").iter().find(|c| c["camera"] == "master") {
                        s.master = string(master, "path");
                    }
                    s.saved_project = s.project.clone();
                    s.edited_at = None;
                    s.status = "Saved".into();
                    s.remember_episode();
                    let per_project = PathBuf::from(&s.cache).join(format!("unsaved-{}.json", s.project_id));
                    let recovery = if per_project.is_file() { per_project } else { PathBuf::from(&s.cache).join("unsaved-workflow.json") };
                    if let Ok(draft) = load_state(&recovery)
                        && draft.project_id == s.project_id
                    {
                        if draft.project["revision"] == s.project["revision"] && draft.project != s.project {
                            s.project = draft.project;
                            s.edited_at = Some(Instant::now());
                            s.status = "Recovered unsaved corrections".into();
                        }
                        if let Err(error) = std::fs::rename(&recovery, PathBuf::from(&s.cache).join("recovered-workflow.json")) {
                            s.error = format!("Could not archive recovery: {error}");
                        }
                    }
                    match std::mem::take(&mut s.follow_up).as_str() {
                        "open" => {
                            s.step = s.resume_step();
                            s.projects_open = false;
                            if matches!(s.step, 4 | 5) && !rows(&s.project, "shots").is_empty() {
                                s.review_seek = 0.;
                                s.review_preview(s.step == 5);
                            } else if !rows(&s.project, "activity").is_empty() {
                                s.load_samples();
                            }
                        }
                        "open_saved_edit" => {
                            s.assembled = true;
                            s.projects_open = false;
                            s.step = 6;
                            app.set_workspace("Editing");
                        }
                        "diarize" => s.job("diarize", json!({})),
                        "samples" => s.load_samples(),
                        "cameras" => s.step = 3,
                        "review" => s.review_preview(s.step == 5 || (s.step == 3 && s.review_is_edit)),
                        _ => {}
                    }
                }
                "created" => {
                    s.projects_open = false;
                    s.project_id = string(&value, "id");
                    s.project = value;
                    let master = s.master.clone();
                    s.job("import", json!({"master":master,"A":[],"B":[]}));
                }
                "storage" => {
                    s.storage = value;
                }
                "deleted" => {
                    s.project = Value::Null;
                    s.saved_project = Value::Null;
                    s.project_id.clear();
                    s.storage = Value::Null;
                    s.confirm_delete = false;
                    s.review_item = None;
                    s.assembled = false;
                    s.step = 1;
                    s.status = "Project deleted · Source recordings kept".into();
                    let _ = std::fs::remove_file(PathBuf::from(&s.cache).join("last-episode.txt"));
                }
                "projects" => {
                    s.episodes = value["projects"].as_array().cloned().unwrap_or_default();
                    s.project_home = string(&value["storage"], "projects_home");
                }
                "capabilities" => {
                    s.setup.checked = true;
                    s.setup.capabilities = value;
                    s.status.clear();
                    let advance = std::mem::take(&mut s.setup.advance_on_ready);
                    if s.projects_open {
                        s.list_projects();
                    }
                    if s.setup.ready() && s.step == 0 && advance {
                        s.step = 1;
                    }
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
                        s.saved_project = s.project.clone();
                        s.edited_at = None;
                    }
                    if let Some(error) = value["error"].as_str() {
                        s.error = display_error(error.into());
                        s.auto_identify = false;
                        s.follow_up.clear();
                    } else {
                        s.job = Some(string(&value["job"], "id"));
                        s.status = format!("{} queued", job_label(&s.job_kind));
                    }
                }
                "cancel" => {
                    s.status = "Cancelling…".into();
                }
                "poll" => {
                    let state = string(&value, "state");
                    let detail = &value["detail"];
                    let stage = detail["stage"].as_str().unwrap_or_else(|| job_label(&s.job_kind));
                    s.status = if let (Some(done), Some(total)) = (detail["completed"].as_u64(), detail["total"].as_u64()) {
                        format!("{stage} · {done}/{total} {} · {:.0}%", detail["units"].as_str().unwrap_or("segments"), number(&value, "progress") * 100.)
                    } else if state == "running" {
                        stage.into()
                    } else {
                        format!("{stage} · {state}")
                    };
                    if ["succeeded", "failed", "cancelled", "interrupted"].contains(&state.as_str()) {
                        let jid = s.job.take().unwrap_or_default();
                        if state == "succeeded" {
                            if matches!(s.job_kind.as_str(), "sample" | "reaction_preview" | "sync_preview" | "reference_frame") {
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
                            } else if s.job_kind == "reference_suggestions" {
                                s.reference_candidates = value["result"]["reference_candidates"].clone();
                                s.reference_thumbnails.clear();
                                s.reference_candidate_clip.clear();
                                s.status = "Choose a reference".into();
                                if let Some(issues) = value["result"]["issues"].as_object()
                                    && !issues.is_empty()
                                {
                                    s.error = format!(
                                        "{} host recording(s) need a clearer reference. Move to a visible host face and use current frame, or choose a reference screenshot. Successful references were kept.",
                                        issues.len()
                                    );
                                }
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
                                if s.job_kind == "import" {
                                    s.step = 2;
                                    if std::mem::take(&mut s.auto_identify) {
                                        s.follow_up = "diarize".into();
                                    }
                                } else if s.job_kind == "diarize" {
                                    s.step = 2;
                                    s.follow_up = "samples".into();
                                }
                                if matches!(s.job_kind.as_str(), "reactions" | "reaction_select") {
                                    s.recalibrating = false;
                                    s.follow_up = "review".into();
                                }
                                if s.job_kind == "transcribe" {
                                    s.step = 4;
                                    s.follow_up = "review".into();
                                }
                                if s.job_kind == "sync" {
                                    s.step = 3;
                                    s.follow_up = "review".into();
                                }
                                if s.job_kind == "framing" {
                                    s.status = format!("Framing suggested for {} recordings · review before confirming", value["result"]["framed"]);
                                    s.follow_up = "review".into();
                                }
                                if s.job_kind == "shots" {
                                    s.step = 5;
                                    s.follow_up = "review".into();
                                }
                                if s.job_kind == "attach_cameras" {
                                    s.camera_a.clear();
                                    s.camera_b.clear();
                                }
                                s.refresh();
                            }
                        } else {
                            s.error = display_error(string(&value, "error"));
                            s.auto_identify = false;
                            s.follow_up.clear();
                        }
                    }
                }
                "play" => {
                    s.review_active = false;
                    app.stop();
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
                            app.source_playback.clock.looping = false;
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
                "review_assemble" => {
                    s.project = value["project"].clone();
                    s.saved_project = s.project.clone();
                    s.edited_at = None;
                    s.remember_episode();
                    if let Some(error) = value["error"].as_str() {
                        s.error = error.into();
                    } else {
                        // Finish the old sequence's automation and audio before replacing its strips.
                        app.stop();
                        app.stop_source();
                        match app.session.execute("podcast.assemble", value["plan"].clone()).and_then(|result| {
                            let item = result["sequence"].as_u64().ok_or_else(|| filmcraft_engine::EngineError::Other("Review sequence missing".into()))?;
                            s.review_item = Some(item);
                            s.review_active = true;
                            s.review_is_edit = value["plan"]["review_edit"] == true;
                            app.stop_source();
                            app.ui.program.zoom = None;
                            if let Some(track) = app.session.active_sequence().and_then(|q| q.audio_tracks.first()) {
                                let strip = track.id.0;
                                app.session.execute("mixer.setStrip", json!({"strip":strip,"muted":s.monitor_muted}))?;
                                app.session.execute("mixer.setValue", json!({"strip":strip,"lane":"volume","value":s.monitor_volume_db.clamp(-60.,6.)}))?;
                            }
                            app.session.execute("playhead.set", json!({"seconds":s.review_seek}))
                        }) {
                            Ok(_) => {
                                s.status = "Review ready · Master audio".into();
                                if s.review_autoplay {
                                    s.review_autoplay = false;
                                    app.play(1.);
                                }
                            }
                            Err(error) => s.error = error.to_string(),
                        }
                    }
                }
                "assemble" => {
                    if value["project"].is_object() {
                        s.project = value["project"].clone();
                        s.saved_project = s.project.clone();
                        s.edited_at = None;
                    }
                    if let Some(error) = value["error"].as_str() {
                        s.error = error.into();
                    } else {
                        // Finish the old sequence's automation and audio before replacing its strips.
                        app.stop();
                        app.stop_source();
                        let native_path = value["storage"]["path"].as_str().map(|folder| PathBuf::from(folder).join("edit.fcproj"));
                        match app.session.execute("podcast.assemble", value.get("plan").cloned().unwrap_or(value)) {
                            Ok(_) => {
                                s.assembled = true;
                                s.step = 6;
                                app.ui.mode = Mode::Edit;
                                app.ui.text_tab = "Transcript".into();
                                s.status = "Edit ready".into();
                                if !s.remote
                                    && let Some(path) = native_path.filter(|p| !p.exists())
                                {
                                    match app.session.execute("file.saveAs", json!({"path":path})) {
                                        Ok(_) => {
                                            if !s.project["reaction_calibration"].is_object() {
                                                s.project["reaction_calibration"] = json!({});
                                            }
                                            s.project["reaction_calibration"]["native_project"] = json!(path);
                                            s.project["reaction_calibration"]["native_ready"] = json!(true);
                                            s.edited_at = Some(Instant::now());
                                        }
                                        Err(error) => s.error = format!("Edit opened; saving project failed: {error}"),
                                    }
                                }
                                app.set_workspace("Editing");
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
        return Ok(
            json!({"projects_open":s.projects_open,"projects_home":s.project_home,"audio_error":app.playback.audio_error,"audio_output_open":s.audio_output.open,"audio_test_status":s.audio_output.message,"busy":s.busy(),"step":s.step,"project_id":s.project_id,"project":s.project,
            "master":s.master,"camera_a":s.camera_a,"camera_b":s.camera_b,
            "compute_device":s.compute_device,"samples":s.samples,"job":s.job,"regeneration_pending":s.pending_regeneration.as_ref().map(|v| &v.0),
            "setup":{"ready":s.setup.ready(),"busy":s.setup.busy(),"stage":s.setup.stage,"error":s.setup.error},
            "job_kind":s.job_kind,"status":s.status,"error":s.error,"assembled":s.assembled,
            "actions":["export_metadata","import_metadata","create","refresh","samples","save","review_playback","accept_transcript","lock_roles","attach_cameras","diarize","sync","transcribe","shots","reactions","reaction_select","reference_suggestions","framing","sample","sync_preview","reference_frame","reaction_preview","transliterate","open_edit","cancel","keep_results","confirm_regenerate"]}),
        );
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
            updated.setup = std::mem::take(&mut s.setup);
            updated.ai_setup = std::mem::take(&mut s.ai_setup);
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
                    s.import_audio(options["identify"].as_bool().unwrap_or(true));
                }
                "keep_results" => s.pending_regeneration = None,
                "confirm_regenerate" => {
                    let (kind, mut options) = s.pending_regeneration.take().ok_or("No regeneration requested")?;
                    if kind == "diarize" {
                        options["replace_activity"] = json!(true);
                    }
                    s.run_job(&kind, options);
                }
                "refresh" => s.refresh(),
                "save" => s.save(),
                "export_metadata" => s.export_metadata(options["path"].as_str().filter(|p| !p.is_empty()).ok_or("Choose a metadata path")?.to_owned()),
                "import_metadata" => s.import_metadata(options["path"].as_str().filter(|p| !p.is_empty()).ok_or("Choose a metadata path")?.to_owned()),
                "review_playback" => s.review_preview(options["edit"].as_bool().unwrap_or(false)),
                "accept_transcript" => {
                    for caption in s.project["captions"].as_array_mut().into_iter().flatten() {
                        caption["reviewed"] = json!(true);
                    }
                    if rows(&s.project, "shots").is_empty() {
                        s.job("shots", json!({}));
                    } else {
                        s.step = 5;
                        s.remember_view();
                        s.save();
                    }
                }
                "samples" => {
                    let id = s.project_id.clone();
                    s.start("samples", move |c| c.get(&format!("/v1/projects/{id}/speaker-samples")));
                }
                "lock_roles" => {
                    lock_roles(s, p["host"].as_str().ok_or("Missing host speaker ID")?)?;
                    s.follow_up = "cameras".into();
                    s.save();
                }
                "attach_cameras" => s.job(action, json!({"A":s.camera_a,"B":s.camera_b})),
                "diarize"
                | "sync"
                | "transcribe"
                | "shots"
                | "reactions"
                | "reaction_select"
                | "reference_suggestions"
                | "framing"
                | "sample"
                | "sync_preview"
                | "reference_frame"
                | "reaction_preview"
                | "transliterate" => s.job(action, options),
                "open_edit" => {
                    let id = s.project_id.clone();
                    let project = s.project.clone();
                    s.start("assemble", move |c| {
                        let saved = c.request(reqwest::Method::PUT, &format!("/v1/projects/{id}"), Some(&project))?;
                        match c.get(&format!("/v1/projects/{id}/filmcraft")) {
                            Ok(plan) => Ok(json!({"plan":plan,"project":saved,"storage":c.get(&format!("/v1/projects/{id}/storage"))?})),
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
    if s.token.is_empty() && !s.remote {
        s.token = local_token();
    }
    s.audio_output.tick(app, ctx);
    poll(app, &mut s);
    if let Some(text) = s.diagnostics_text.take() {
        ctx.copy_text(text);
    }
    let setup_finished = s.setup.poll();
    s.ai_setup.poll();
    let initial_setup = std::mem::take(&mut s.setup.check_on_start);
    if initial_setup {
        s.setup.advance_on_ready = true;
    }
    let refresh_capabilities = !s.busy() && std::mem::take(&mut s.refresh_capabilities);
    if (initial_setup || setup_finished || refresh_capabilities) && !s.busy() {
        if !s.setup.checked {
            s.step = 0;
        }
        s.start("capabilities", |c| {
            c.wait_ready()?;
            c.get("/v1/capabilities")
        });
    }
    if !s.busy()
        && s.project != s.saved_project
        && s.edited_at.is_some_and(|time| time.elapsed() >= Duration::from_secs(if s.error.is_empty() { 1 } else { 5 }))
    {
        s.edited_at = None;
        s.save();
    }
    if s.edited_at.is_some() {
        ctx.request_repaint_after(Duration::from_millis(200));
    }
    if s.busy() || s.setup.busy() || s.ai_setup.busy() {
        ctx.request_repaint_after(Duration::from_millis(100));
    }
    app.podcast = s;
}

fn app_controls(app: &mut FilmcraftApp, ui: &mut egui::Ui, s: &mut State) {
    if button(app, ui, "updates", "Updates") {
        app.updates.open = true;
        app.updates.check_requested = true;
    }
    if button(app, ui, "connect_ai", "Connect AI") {
        s.setup.ai_open = !s.setup.ai_open;
    }
    let settings = ui.menu_button("Settings", |ui| {
        if button(app, ui, "models", "Models") {
            s.projects_open = false;
            s.step = 0;
            s.start("capabilities", |c| c.get("/v1/capabilities"));
            ui.close();
        }
        if button(app, ui, "audio_output", "Audio output") {
            s.audio_output.open = true;
            ui.close();
        }
        if !s.project_id.is_empty() && button(app, ui, "project_folder", "Project files…") {
            let id = s.project_id.clone();
            s.start("storage", move |c| c.get(&format!("/v1/projects/{id}/storage")));
            ui.close();
        }
        if !s.project_id.is_empty() && !s.busy() && button(app, ui, "diagnostics", "Copy diagnostics") {
            let id = s.project_id.clone();
            s.start("diagnostics", move |c| c.get(&format!("/v1/projects/{id}/diagnostics")));
            ui.close();
        }
    });
    app.auto.add_response("podcast.settings", &settings.response, "Application settings");
}

pub fn show(app: &mut FilmcraftApp, ui: &mut egui::Ui, rect: Rect) {
    let mut s = std::mem::take(&mut app.podcast);
    if let Err(error) = validate_project(&s.project) {
        s.error = error;
        s.step = 0;
    }
    if s.projects_open && s.project_id.is_empty() {
        let age = s.startup.get_or_insert_with(Instant::now).elapsed().as_secs_f32();
        if age < 0.65 {
            ui.vertical_centered(|ui| {
                ui.add_space((rect.height() * 0.30).max(0.));
                ui.label(egui::RichText::new("YouCraft").size(42.).strong());
                ui.label("Your conversation. Your story.");
                ui.add_space(20.);
                ui.add(egui::ProgressBar::new((age / 0.65).clamp(0., 1.)).desired_width(180.));
            });
            ui.ctx().request_repaint_after(Duration::from_millis(16));
            app.podcast = s;
            return;
        }
    }
    let before_edit = s.project.clone();
    let guided = !s.assembled || s.projects_open;
    if guided {
        ui.painter().rect_filled(rect.shrink(8.), 12., egui::Color32::from_rgb(20, 25, 33));
    }
    let inset = if guided { 16. } else { 12. };
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink(inset)).layout(egui::Layout::top_down(egui::Align::Min)));
    let ui = &mut child;
    if guided {
        ui.style_mut().override_font_id = Some(egui::FontId::proportional(15.));
        ui.spacing_mut().item_spacing = egui::vec2(8., 4.);
        ui.spacing_mut().button_padding = egui::vec2(8., 4.);
        ui.spacing_mut().text_edit_width = 520.;
        ui.visuals_mut().widgets.inactive.weak_bg_fill = egui::Color32::from_rgb(35, 43, 55);
    }
    let narrow_header = ui.available_width() < 760.;
    ui.horizontal(|ui| {
        if let Some(logo) = crate::brand::wordmark(ui.ctx(), true) {
            ui.add(
                egui::Image::new(&logo).uv(egui::Rect::from_min_max(egui::pos2(0.28, 0.20), egui::pos2(0.72, 0.62))).fit_to_exact_size(egui::vec2(28., 28.)),
            );
        }
        ui.label(egui::RichText::new("YouCraft").size(22.).strong());
        ui.weak(format!("v{}", include_str!("../../../packaging/youcraft-version.txt").trim()));
        if button(app, ui, "episodes", "Projects") {
            if !s.busy() {
                app.stop();
                app.stop_source();
                let saved = if s.assembled && app.session.is_dirty() {
                    if app.session.path.is_some() {
                        app.session.execute("file.save", json!({})).map(|_| ()).map_err(|e| e.to_string())
                    } else {
                        Err("Save the native edit before switching projects.".into())
                    }
                } else {
                    Ok(())
                };
                if let Err(error) = saved {
                    s.error = error;
                } else {
                    s.projects_open = true;
                    app.set_workspace("Podcast");
                    s.list_projects();
                }
            } else {
                s.error = "Finish or cancel the current task before switching projects".into();
            }
        }
        if s.assembled && button(app, ui, "editor", "Editor") {
            app.set_workspace("Editing");
        }
        if !narrow_header {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| app_controls(app, ui, &mut s));
        }
    });
    if narrow_header {
        ui.horizontal(|ui| app_controls(app, ui, &mut s));
    }
    if s.storage.is_object() {
        egui::Window::new("Project folder").default_width(560.).vscroll(true).show(ui.ctx(), |ui| {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label("Project name");
                    let mut name = string(&s.project, "name");
                    let r = field(app, ui, "project_name", &mut name);
                    if r.changed() && !name.trim().is_empty() && name.chars().count() <= 200 {
                        s.project["name"] = json!(name.trim());
                        s.name = name.trim().into();
                    }
                });
                ui.label(string(&s.storage, "path"));
                if s.storage["backup_current"] != true {
                    ui.colored_label(egui::Color32::LIGHT_RED, "Folder backup needs attention · Database copy is saved");
                }
                ui.horizontal_wrapped(|ui| {
                    if button(app, ui, "open_project_folder", "Open folder") {
                        let path = PathBuf::from(string(&s.storage, "path"));
                        open_path(app, &mut s, &path);
                    }
                    if !s.busy()
                        && button(app, ui, "change_project_folder", "Change folder")
                        && let Some(path) = app.hooks.pick_folder.as_mut().and_then(|pick| pick())
                    {
                        let id = s.project_id.clone();
                        s.start("storage", move |c| c.post(&format!("/v1/projects/{id}/storage"), json!({"path":path})));
                    }
                    if !s.busy()
                        && button(app, ui, "export_metadata", "Export metadata…")
                        && let Some(path) = app.hooks.pick_save.as_mut().and_then(|pick| pick("project.betun.json"))
                    {
                        s.export_metadata(path);
                    }
                    if !s.busy()
                        && button(app, ui, "import_metadata", "Import corrections…")
                        && let Some(path) = app.hooks.pick_files.as_mut().and_then(|pick| pick(&["json"]).into_iter().next())
                    {
                        s.import_metadata(path);
                    }
                    if button(app, ui, "close_project_folder", "Close") {
                        s.storage = Value::Null;
                    }
                });
                if let Some(previous) = s.storage["previous_path"].as_str() {
                    ui.label(format!("Previous copy: {previous}"));
                }
                let response = ui.checkbox(&mut s.confirm_delete, "Delete project and generated files");
                app.auto.add_response("podcast.confirm_delete", &response, "Confirm project deletion");
                if s.confirm_delete && !s.busy() && button(app, ui, "delete_project", "Delete project") {
                    let id = s.project_id.clone();
                    let revision = s.project["revision"].clone();
                    s.start("deleted", move |c| c.request(reqwest::Method::DELETE, &format!("/v1/projects/{id}"), Some(&json!({"revision":revision}))));
                }
            });
        });
    }
    if s.setup.ai_open {
        let mut open = true;
        egui::Window::new("Connect AI")
            .open(&mut open)
            .default_width(600.)
            .max_width(rect.width() - 60.)
            .max_height(rect.height() - 80.)
            .vscroll(true)
            .show(ui.ctx(), |ui| ai_connection(app, ui, &mut s));
        s.setup.ai_open = open;
    }
    s.audio_output.show(app, ui.ctx());
    if let Some((kind, mut options)) = s.pending_regeneration.clone() {
        egui::Window::new("Regenerate saved results?").collapsible(false).resizable(false).show(ui.ctx(), |ui| {
            ui.label(format!("{} already has saved results.", job_label(&kind)));
            ui.label("Regeneration may change dependent edits. Navigation never regenerates results.");
            ui.label("Reviewed/locked transcript text is preserved. Previous project revisions remain in undo history.");
            ui.horizontal(|ui| {
                if button(app, ui, "keep_results", "Keep saved results") {
                    s.pending_regeneration = None;
                }
                if button(app, ui, "confirm_regenerate", "Regenerate") && !s.busy() {
                    if kind == "diarize" {
                        options["replace_activity"] = json!(true);
                    }
                    s.pending_regeneration = None;
                    s.run_job(&kind, options);
                }
            });
        });
    }
    if s.projects_open {
        ui.separator();
        ui.add_space(24.);
        ui.heading("Projects");
        ui.add_space(16.);
        ui.horizontal_wrapped(|ui| {
            if !s.project_home.is_empty() {
                ui.weak(&s.project_home);
                if button(app, ui, "open_projects_folder", "Open folder") {
                    let home = s.project_home.clone();
                    open_path(app, &mut s, Path::new(&home));
                }
            }
            if !s.busy()
                && button(app, ui, "choose_projects_folder", "Change default…")
                && let Some(path) = app.hooks.pick_folder.as_mut().and_then(|pick| pick())
            {
                s.start("projects", move |c| {
                    let storage = c.post("/v1/storage", json!({"path":path}))?;
                    Ok(json!({"storage":storage,"projects":c.get("/v1/projects")?}))
                });
            }
        });
        let can_switch = !s.busy() && s.edited_at.is_none() && s.project == s.saved_project;
        ui.add_enabled_ui(can_switch, |ui| {
            if button(app, ui, "new_project", "New project") {
                app.stop();
                app.stop_source();
                app.session.state.active_sequence = None;
                s.reset_episode();
            }
        });
        if !can_switch {
            ui.label(if s.busy() { "Finishing current work…" } else { "Saving changes…" });
        }
        ui.add_space(16.);
        ui.strong("Your projects");
        let mut selected = None;
        let mut selected_edit = None;
        egui::ScrollArea::vertical().id_salt("project-list").show(ui, |ui| {
            for episode in &s.episodes {
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.strong(string(episode, "name"));
                            ui.weak(format!("{} · {:.1} min", episode["stage"].as_str().unwrap_or("Project"), number(episode, "duration") / 60.));
                            if let Some(path) = episode.get("project_folder").and_then(Value::as_str) {
                                ui.small(path);
                            }
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_enabled_ui(can_switch, |ui| {
                                if !s.remote
                                    && episode["native_ready"] == true
                                    && let Some(path) = episode["native_project"].as_str()
                                    && button(app, ui, &format!("episode_edit.{}", string(episode, "id")), "Open edit")
                                {
                                    selected_edit = Some((string(episode, "id"), path.to_owned()));
                                }
                                if button(app, ui, &format!("episode.{}", string(episode, "id")), "Open project") {
                                    selected = Some(string(episode, "id"));
                                }
                            });
                        });
                    });
                });
            }
        });
        if let Some((id, path)) = selected_edit {
            app.stop();
            app.stop_source();
            match app.session.execute("file.open", json!({"path":path})) {
                Ok(_) => {
                    s.reset_episode();
                    s.project_id = id;
                    s.follow_up = "open_saved_edit".into();
                    s.refresh();
                }
                Err(error) => s.error = format!("Could not open saved edit: {error}"),
            }
        }
        if let Some(id) = selected {
            app.stop();
            app.stop_source();
            app.session.state.active_sequence = None;
            s.reset_episode();
            s.projects_open = true;
            s.project_id = id;
            s.follow_up = "open".into();
            s.refresh();
        }
        if s.episodes.is_empty() && !s.busy() {
            ui.label("No saved projects");
        }
        if !s.error.is_empty() {
            ui.colored_label(egui::Color32::LIGHT_RED, &s.error);
        }
        if !s.busy() && button(app, ui, "refresh_projects", "Refresh") {
            s.list_projects();
        }
        if !s.project_id.is_empty() && button(app, ui, "back_to_project", "Back to project") {
            s.projects_open = false;
            if s.assembled {
                app.set_workspace("Editing");
            }
        }
        app.podcast = s;
        return;
    }
    if s.setup.busy() {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(&s.setup.stage);
            if button(app, ui, "live_setup_log", "Setup log") {
                open_path(app, &mut s, &crate::podcast_setup::log_path());
            }
            if button(app, ui, "cancel_setup", "Cancel setup") {
                s.setup.cancel();
            }
        });
        let progress = &s.setup.progress;
        if let (Some(done), Some(total)) = (progress["downloaded_bytes"].as_f64(), progress["total_bytes"].as_f64())
            && total > 0.
        {
            let speed = progress["bytes_per_second"].as_f64().unwrap_or(0.) / 1_048_576.;
            ui.add(egui::ProgressBar::new((done / total).clamp(0., 1.) as f32).text(format!(
                "{:.0}% · {:.1} / {:.1} GB · {:.1} MB/s",
                done / total * 100.,
                done / 1_073_741_824.,
                total / 1_073_741_824.,
                speed
            )));
            if let Some(file) = progress["file"].as_str() {
                ui.small(file);
            }
            if progress["idle_seconds"].as_u64().unwrap_or(0) >= 30 && s.setup.stage.starts_with("Downloading") {
                ui.label("Waiting for data · Retry keeps downloaded files");
            }
        }
        ui.ctx().request_repaint_after(Duration::from_millis(500));
    }
    if !s.setup.error.is_empty() {
        ui.colored_label(egui::Color32::LIGHT_RED, &s.setup.error);
    }
    ui.horizontal_wrapped(|ui| {
        for (i, label) in [(1, "Audio"), (2, "Speakers"), (3, "Cameras"), (4, "Transcript"), (5, "Reactions"), (6, "Export")] {
            let available = match i {
                1 => true,
                2 => number(&s.project, "duration") > 0.,
                3 => s.project["settings"]["roles_locked"] == true,
                4 => !rows(&s.project, "activity").is_empty() || !rows(&s.project, "captions").is_empty(),
                5 => !rows(&s.project, "shots").is_empty() || rows(&s.project, "clips").iter().any(|c| c["camera"] != "master"),
                _ => s.assembled || !rows(&s.project, "shots").is_empty(),
            };
            let r = ui.add_enabled_ui(available, |ui| ui.selectable_label(s.step == i, label)).inner;
            app.auto.add(&format!("podcast.step.{i}"), r.rect, label);
            if r.clicked() {
                if s.edited_at.is_some() && !s.busy() {
                    s.save();
                }
                let from_cameras = s.step == 3;
                s.step = i;
                s.remember_view();
                if matches!(i, 3..=5)
                    && (from_cameras || i == 3 || !s.review_active || s.review_is_edit != (i == 5) || s.review_clip.is_some() || s.review_reaction.is_some())
                {
                    app.stop();
                    if i == 3 {
                        s.review_is_edit = false;
                    }
                    s.review_seek = app.session.playhead().seconds();
                    if s.busy() {
                        s.follow_up = "review".into();
                    } else {
                        s.review_preview(i == 5);
                    }
                }
            }
        }
    });
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        if !s.project_id.is_empty() {
            ui.small(string(&s.project, "name"));
        }
        if !s.status.is_empty() {
            ui.weak(&s.status);
        }
    });
    if !s.error.is_empty() {
        ui.colored_label(egui::Color32::LIGHT_RED, &s.error);
    }
    if !s.error.is_empty()
        && !s.busy()
        && let Some((kind, options)) = s.retry_job.clone()
        && matches!(kind.as_str(), "diarize" | "transcribe")
    {
        ui.horizontal_wrapped(|ui| {
            if button(app, ui, "retry_gpu", "Retry on GPU") {
                s.compute_device = "cuda".into();
                s.run_job(&kind, options.clone());
            }
            if kind == "transcribe" && button(app, ui, "retry_smaller", "Retry GPU · shorter segments") {
                let seconds = s.project["settings"]["transcription_segment_seconds"].as_f64().unwrap_or(28.);
                s.project["settings"]["transcription_segment_seconds"] = json!((seconds / 2.).max(8.));
                s.compute_device = "cuda".into();
                s.run_job(&kind, options.clone());
            }
            if button(app, ui, "retry_cpu", "Use CPU · slower") {
                s.compute_device = "cpu".into();
                s.run_job(&kind, options.clone());
            }
        });
    }
    if s.pending.is_some() || s.job.is_some() {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.add_enabled_ui(s.job.is_some() && s.pending.is_none(), |ui| {
                if button(app, ui, "cancel", "Cancel") {
                    let id = s.job.clone().unwrap_or_default();
                    s.start("cancel", move |c| c.post(&format!("/v1/jobs/{id}/cancel"), json!({})));
                }
            });
        });
        ui.ctx().request_repaint_after(Duration::from_millis(100));
    }
    if matches!(s.step, 3..=5) {
        let bounds = ui.available_rect_before_wrap();
        let (min_fraction, max_fraction) = if s.step == 3 { (0.32, 0.70) } else { (0.58, 0.82) };
        let fraction = if s.step == 3 { s.camera_video_fraction } else { s.video_fraction }.clamp(min_fraction, max_fraction);
        let height = if s.focus_video { bounds.height() } else { (bounds.height() * fraction).max(100.).min(bounds.height().max(0.)) };
        let top = Rect::from_min_max(bounds.min, egui::pos2(bounds.right(), bounds.top() + height));
        let bottom = Rect::from_min_max(egui::pos2(bounds.left(), top.bottom() + 8.), bounds.max);
        let mut preview = ui.new_child(egui::UiBuilder::new().max_rect(top));
        review_player(app, &mut preview, &mut s);
        if !s.focus_video {
            let divider = Rect::from_min_max(egui::pos2(bounds.left(), top.bottom()), egui::pos2(bounds.right(), top.bottom() + 8.));
            let drag = ui.interact(divider, ui.id().with("video-height"), egui::Sense::drag()).on_hover_cursor(egui::CursorIcon::ResizeVertical);
            app.auto.add_response("podcast.video_height", &drag, "Drag to resize video area");
            ui.painter().line_segment([divider.left_center(), divider.right_center()], egui::Stroke::new(2., egui::Color32::from_gray(70)));
            if drag.dragged()
                && let Some(pos) = drag.interact_pointer_pos()
            {
                let fraction = ((pos.y - bounds.top()) / bounds.height().max(1.)).clamp(min_fraction, max_fraction);
                if s.step == 3 {
                    s.camera_video_fraction = fraction;
                } else {
                    s.video_fraction = fraction;
                    s.transcript_video_fraction = fraction;
                }
            }
            let mut controls = ui.new_child(egui::UiBuilder::new().max_rect(bottom));
            if s.step == 5 {
                reactions(app, &mut controls, &mut s);
            } else {
                egui::ScrollArea::vertical().id_salt("podcast-body").show(&mut controls, |ui| {
                    ui.set_min_width(ui.available_width());
                    workflow_step(app, ui, &mut s);
                });
            }
        }
    } else {
        egui::ScrollArea::vertical().id_salt("podcast-body").show(ui, |ui| workflow_step(app, ui, &mut s));
    }
    if s.project != before_edit && !s.project_id.is_empty() && !s.busy() {
        s.edited_at = Some(Instant::now());
        if s.review_active
            && (s.project["clips"] != before_edit["clips"]
                || s.project["reactions"] != before_edit["reactions"]
                || s.project["settings"] != before_edit["settings"]
                || s.project["shots"] != before_edit["shots"])
        {
            app.stop();
            s.review_seek = app.session.playhead().seconds();
            s.follow_up = "review".into();
        }
        s.status = "Saving…".into();
        ui.ctx().request_repaint_after(Duration::from_secs(1));
    }
    app.podcast = s;
}
fn review_player(app: &mut FilmcraftApp, ui: &mut egui::Ui, s: &mut State) {
    ui.horizontal(|ui| {
        if s.step == 5 {
            ui.strong("Output");
            let toggle = ui.toggle_value(&mut s.preview_suggestions, "Preview suggestions");
            app.auto.add_response("podcast.preview_suggestions", &toggle, "Preview pending reactions in output");
            if toggle.changed() && !s.busy() {
                app.stop();
                s.review_seek = app.session.playhead().seconds();
                s.review_preview(true);
            }
            let time = app.session.playhead().seconds();
            let active =
                rows(&s.project, "reactions").iter().position(|r| r["decision"] != "reject" && number(r, "start") <= time + 0.02 && time < number(r, "end"));
            if let Some(index) = active {
                ui.colored_label(egui::Color32::LIGHT_RED, "Reaction");
                let keep = button(app, ui, "active_reaction_keep", "Keep");
                let remove = button(app, ui, "active_reaction_remove", "Remove");
                if keep || remove {
                    s.reaction_undo = Some((s.project_id.clone(), s.project["reactions"].clone(), s.project["reaction_analysis"]["decisions"].clone()));
                }
                if (keep || remove)
                    && let Some(r) = s.project["reactions"].as_array_mut().and_then(|rows| rows.get_mut(index))
                {
                    r["decision"] = json!(if keep { "keep" } else { "reject" });
                }
            }
        } else {
            ui.strong(if s.step == 3 && s.review_is_edit { "Split-screen framing" } else { "Host · Guest" });
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let focus = ui.toggle_value(&mut s.focus_video, "Focus video");
            app.auto.add_response("podcast.focus_video", &focus, "Focus video");
            if !s.busy() && button(app, ui, "review_playback", "Reload") {
                s.review_seek = app.session.playhead().seconds();
                s.review_preview(s.step == 5);
            }
            let menu = ui.menu_button("Audio", |ui| {
                if let Some(track) = app.session.active_sequence().and_then(|q| q.audio_tracks.first()) {
                    let id = track.id.0;
                    let mut volume = track.volume_db;
                    let mut mute = track.muted;
                    if ui.checkbox(&mut mute, "Mute").changed() {
                        s.monitor_muted = mute;
                        if let Err(e) = app.session.execute("mixer.setStrip", json!({"strip":id,"muted":mute})) {
                            s.error = e.to_string();
                        }
                    }
                    if ui.add(egui::Slider::new(&mut volume, -60.0..=6.0).text("Volume dB")).changed() {
                        s.monitor_volume_db = volume;
                        if let Err(e) = app.session.execute("mixer.setValue", json!({"strip":id,"lane":"volume","value":volume})) {
                            s.error = e.to_string();
                        }
                    }
                }
                if button(app, ui, "audio_output", "Audio output") {
                    s.audio_output.open = true;
                }
            });
            app.auto.add_response("podcast.monitor_audio", &menu.response, "Monitor audio controls");
        });
    });
    if s.step == 5 {
        let duration = number(&s.project, "duration").max(1.);
        let (bar, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 18.), egui::Sense::hover());
        ui.painter().rect_filled(bar, 3., egui::Color32::from_gray(35));
        for shot in rows(&s.project, "shots").iter().filter(|q| q["layout"] == "both") {
            let left = bar.left() + (number(shot, "start") / duration) as f32 * bar.width();
            let right = bar.left() + (number(shot, "end") / duration) as f32 * bar.width();
            let region = Rect::from_min_max(egui::pos2(left, bar.bottom() - 4.), egui::pos2(right.max(left + 2.), bar.bottom()));
            ui.painter().rect_filled(region, 1., egui::Color32::from_rgb(80, 155, 220));
            let response = ui.interact(region, ui.id().with(string(shot, "id")), egui::Sense::click());
            app.auto.add_response(&format!("podcast.overlap_marker.{}", string(shot, "id")), &response, "Split screen");
            if response.clicked() {
                app.stop();
                app.seek_program(filmcraft_time::Tick::from_seconds_f64(number(shot, "start")));
            }
            response.on_hover_text(format!("Split screen · {:.1}–{:.1}s", number(shot, "start"), number(shot, "end")));
        }
        for r in rows(&s.project, "reactions") {
            let a = bar.left() + (number(&r, "start") / duration) as f32 * bar.width();
            let b = bar.left() + (number(&r, "end") / duration) as f32 * bar.width();
            let region = Rect::from_min_max(egui::pos2(a, bar.top()), egui::pos2(b.max(a + 4.), bar.bottom() - 5.));
            if r["decision"] == "reject" {
                continue;
            }
            let color = match r["decision"].as_str() {
                Some("keep") => egui::Color32::from_rgb(70, 180, 110),
                Some("reject") => egui::Color32::from_gray(70),
                _ => egui::Color32::from_rgb(230, 90, 90),
            };
            ui.painter().rect_filled(region, 2., color);
            let response = ui.interact(region, ui.id().with(string(&r, "id")), egui::Sense::click());
            app.auto.add_response(&format!("podcast.reaction_marker.{}", string(&r, "id")), &response, "Preview reaction");
            if response.clicked() {
                app.stop();
                s.review_seek = number(&r, "start");
                app.seek_program(filmcraft_time::Tick::from_seconds_f64(s.review_seek));
                if !s.preview_suggestions && r["decision"] == "pending" {
                    s.review_reaction = Some(string(&r, "id"));
                    s.request_review(true, None);
                }
            }
            response.on_hover_text(format!("{:.1}–{:.1}s · {} · {}", number(&r, "start"), number(&r, "end"), string(&r, "decision"), r["cues"]));
        }
    }
    let rect = ui.available_rect_before_wrap();
    let caption_height = if s.step == 4 { 32. } else { 0. };
    let preview = Rect::from_min_max(rect.min, egui::pos2(rect.right(), (rect.bottom() - caption_height).max(rect.top() + 130.)));
    if s.review_active {
        crate::panels::monitor::show_guided(app, ui, preview, crate::panels::monitor::Which::Program);
        if s.step == 5 && s.review_is_edit {
            reaction_overlay(app, ui, s);
        }
        let time = app.session.playhead().seconds();
        if caption_height > 0.
            && let Some(c) = rows(&s.project, "captions").iter().find(|c| number(c, "start") <= time && time < number(c, "end"))
        {
            ui.painter().text(
                egui::pos2(rect.center().x, preview.bottom() + 6.),
                egui::Align2::CENTER_TOP,
                string(c, "text"),
                egui::FontId::proportional(18.),
                egui::Color32::WHITE,
            );
        }
    } else if app.session.state.source_item.is_some() {
        crate::panels::monitor::show_guided(app, ui, preview, crate::panels::monitor::Which::Source);
    } else {
        ui.painter().rect_filled(preview, 6., egui::Color32::from_rgb(10, 15, 20));
        ui.painter().text(
            preview.center(),
            egui::Align2::CENTER_CENTER,
            "Choose a recording preview or Preview split screen",
            egui::FontId::proportional(16.),
            egui::Color32::GRAY,
        );
    }
}

fn reaction_overlay(app: &FilmcraftApp, ui: &egui::Ui, s: &State) {
    let Some((pic, area)) = ui.ctx().data(|d| d.get_temp::<(Rect, Rect)>(egui::Id::new(("program", "picture_geometry")))) else { return };
    let painter = ui.painter().with_clip_rect(area.intersect(ui.clip_rect()));
    let (host_time, guest_time, total) = visibility(&s.project, s.preview_suggestions);
    let duration = total.max(1.);
    for (x, label) in [
        (0.12, format!("HOST\n{:.0}% · {:.1} min", host_time / duration * 100., host_time / 60.)),
        (0.5, "OUTPUT".into()),
        (0.88, format!("GUEST\n{:.0}% · {:.1} min", guest_time / duration * 100., guest_time / 60.)),
    ] {
        painter.text(
            egui::pos2(pic.left() + pic.width() * x, pic.top() + 8.),
            egui::Align2::CENTER_TOP,
            label,
            egui::FontId::proportional(13.),
            egui::Color32::WHITE,
        );
    }
    if !s.show_detection {
        return;
    }
    let time = app.session.playhead().seconds();
    let measurement = s.project["reaction_analysis"]["measurements"]
        .as_array()
        .and_then(|rows| rows.iter().find(|m| number(m, "time") <= time && time - number(m, "time") < 1.));
    let Some(m) = measurement else {
        painter.text(
            egui::pos2(pic.left() + pic.width() * 0.12, pic.bottom() - 12.),
            egui::Align2::CENTER_BOTTOM,
            "Not analyzed",
            egui::FontId::proportional(12.),
            egui::Color32::GRAY,
        );
        return;
    };
    let color = if m["matching"] == true { egui::Color32::LIGHT_GREEN } else { egui::Color32::YELLOW };
    // A sampled diagnostic, never an overlay on the output/export. Positions come from measured landmarks.
    let aspect = m["aspect"].as_f64().unwrap_or(16. / 9.).clamp(0.1, 10.) as f32;
    let h = 540_f32.min(1920. / aspect) * 0.48;
    let w = h * aspect;
    let map = |point: &Value| -> Option<egui::Pos2> {
        let x = point.get(0)?.as_f64()? as f32;
        let y = point.get(1)?.as_f64()? as f32;
        Some(egui::pos2(pic.left() + pic.width() * (230. - w / 2. + x * w) / 1920., pic.top() + pic.height() * (270. - h / 2. + y * h) / 540.))
    };
    if let Some(eyes) = m["eyes"].as_array() {
        for eye in eyes {
            if let Some(pos) = map(eye) {
                painter.circle_stroke(pos, 4., egui::Stroke::new(1.5, color));
            }
        }
    }
    if let Some(pos) = map(&m["nose"]) {
        painter.circle_stroke(pos, 5., egui::Stroke::new(1.5, color));
    }
    let label = if m["detected"] == true {
        let look = match m["look_distance"].as_f64() {
            Some(distance) => format!("Look {:.1} / {:.1} {}", distance, s.tolerance, if m["look_unit"] == "degrees" { "degrees" } else { "% eye width" }),
            None => "Eye distance —".into(),
        };
        let threshold = s.project["reaction_calibration"]["smile_threshold"]
            .as_f64()
            .or_else(|| s.project["reaction_analysis"]["smile_threshold"].as_f64())
            .unwrap_or(0.35);
        format!("{look}\nSmile {:.2} / {:.2} · Pitch {:.0}°", number(m, "smile"), threshold, number(m, "pitch"))
    } else {
        "Face not detected".into()
    };
    painter.text(egui::pos2(pic.left() + pic.width() * 0.12, pic.bottom() - 12.), egui::Align2::CENTER_BOTTOM, label, egui::FontId::proportional(12.), color);
}

fn workflow_step(app: &mut FilmcraftApp, ui: &mut egui::Ui, s: &mut State) {
    ui.add_enabled_ui(s.pending.is_none() && s.job.is_none() && (!s.setup.busy() || s.step == 0), |ui| match s.step {
        0 => connect(app, ui, s),
        1 => audio(app, ui, s),
        2 => speakers(app, ui, s),
        3 => cameras(app, ui, s),
        4 => review(app, ui, s),
        5 => reactions(app, ui, s),
        _ => editing(app, ui, s),
    });
}
fn open_path(app: &mut FilmcraftApp, s: &mut State, path: &Path) {
    if let Some(open) = app.hooks.open_path.as_mut() {
        if let Err(error) = open(&path.to_string_lossy(), false) {
            s.error = error;
        }
    } else {
        s.error = "Opening files is unavailable here".into();
    }
}
fn ai_connection(app: &mut FilmcraftApp, ui: &mut egui::Ui, s: &mut State) {
    ui.group(|ui| {
        ui.strong("Connect AI");
        ui.label("Register tools, restart your AI app, then enable YouCraft.");
        ui.add_enabled_ui(!s.ai_setup.busy(), |ui| {
            ui.horizontal_wrapped(|ui| {
                if button(app, ui, "ai.desktop", "Register Claude app") {
                    s.ai_setup.start("Connect-AI.ps1", "-Client", "desktop");
                }
                for (id, label) in [("claude", "Register Claude CLI"), ("codex", "Register Codex CLI")] {
                    if button(app, ui, &format!("ai.{id}"), label) {
                        s.ai_setup.start("Connect-AI.ps1", "-Client", id);
                    }
                }
                if button(app, ui, "ai.check", "Check tools") {
                    s.ai_setup.start("Check-AI.ps1", "-Client", "desktop");
                }
            });
        });
        if s.ai_setup.busy() { ui.spinner(); }
        if !s.ai_setup.stage.is_empty() { ui.label(&s.ai_setup.stage); }
        if !s.ai_setup.error.is_empty() { ui.colored_label(egui::Color32::LIGHT_RED, &s.ai_setup.error); }
        ui.label("Keep YouCraft open. Claude Code: use a Local session.");
        ui.collapsing("Connection help", |ui| {
            ui.label("Enable YouCraft in Claude’s tools.");
            ui.label("Alternative: install YouCraft.mcpb from Claude Settings > Extensions.");
            if button(app, ui, "ai.folder", "Extension folder")
                && let Ok(path) = std::env::current_exe()
                && let Some(folder) = path.parent()
            {
                open_path(app, s, folder);
            }
            ui.label("Browser chats cannot use this local extension.");
            if button(app, ui, "ai.log", "AI log") {
                let path = crate::podcast_setup::log_path().with_file_name("ai-setup.log");
                open_path(app, s, &path);
            }
        });
        if button(app, ui, "ai_prompt", "Copy first prompt") {
            ui.ctx().copy_text("Use the enabled YouCraft tools. Call ui_control with method podcast.inspect and params {}. Explain the current step. If ui_control is unavailable, stop and tell me to enable the YouCraft desktop extension in this chat; do not search the connector directory. Ask me to choose the host before assigning cameras. Save before editing.".into());
            s.status = "Prompt copied".into();
        }
    });
}

fn connect(app: &mut FilmcraftApp, ui: &mut egui::Ui, s: &mut State) {
    ui.heading("Set up models");
    ui.label("Local processing · CPU or NVIDIA");
    for (key, label) in [("diarize", "Speaker model"), ("transcribe", "Odia transcript model"), ("reactions", "Host reaction model")] {
        let ready = s.setup.capabilities["installed_providers"][key] == true;
        ui.label(format!("{}  {}", if ready { "✓" } else { "○" }, label));
    }
    let media_ready = s.setup.capabilities["media"]["ffmpeg"] == true && s.setup.capabilities["media"]["ffprobe"] == true;
    ui.label(format!("{}  Media tools", if media_ready { "✓" } else { "○" }));
    ui.label("Downloads require internet and several GB of space.");
    let vision_ready = s.setup.capabilities["installed_providers"]["reactions"] == true;
    ui.label(if vision_ready { "✓  Reaction model" } else { "○  Reaction model · CPU" });
    ui.add_enabled_ui(!s.setup.busy(), |ui| {
        if button(app, ui, "setup_vision", "Install reactions") {
            s.setup.start("Setup-vision.ps1", "-Mode", "cpu");
        }
    });
    if button(app, ui, "model_access", "Odia model access") {
        if let Some(open) = app.hooks.open_url.as_mut() {
            if let Err(error) = open("https://huggingface.co/bodhan-ai/indic-transcribe-core") {
                s.error = error;
            }
        } else {
            s.error = "Browser opening unavailable".into();
        }
    }
    ui.label("Hugging Face read token (if required)");
    let r = ui.add(egui::TextEdit::singleline(&mut s.setup.hf_token).password(true));
    app.auto.add_response("podcast.hf_token", &r, "Hugging Face token");
    ui.add_enabled_ui(!s.setup.busy(), |ui| {
        ui.horizontal(|ui| {
            if button(app, ui, "setup_cpu", "Install all models · CPU") {
                s.setup.start("Setup-models.ps1", "-Mode", "cpu");
            }
            if button(app, ui, "setup_gpu", "Install all models · NVIDIA") {
                s.setup.start("Setup-models.ps1", "-Mode", "nvidia");
            }
        });
    });
    if !s.setup.stage.is_empty() {
        ui.label(&s.setup.stage);
    }
    if button(app, ui, "setup_log", "Setup log")
        && let Some(root) = std::env::var_os("LOCALAPPDATA")
    {
        open_path(app, s, &PathBuf::from(root).join("YouCraft/setup.log"));
    }
    if button(app, ui, "continue_audio", "Continue to audio") {
        s.step = 1;
    }
    ui.separator();
    if button(app, ui, "connection_test", "Check setup") {
        s.start("capabilities", move |c| c.get("/v1/capabilities"));
    }
    ui.collapsing("Advanced", |ui| {
        ui.label("Episode ID");
        field(app, ui, "project_id", &mut s.project_id);
        if button(app, ui, "resume", "Open project") {
            s.refresh();
        }
        if button(app, ui, "list_projects", "Saved episodes") {
            s.projects_open = true;
            s.list_projects();
        }
        if button(app, ui, "save_connection", "Save workflow") {
            let path = PathBuf::from(&s.cache).join("workflow.json");
            let result = std::fs::create_dir_all(&s.cache)
                .map_err(|e| e.to_string())
                .and_then(|_| serde_json::to_vec_pretty(&s).map_err(|e| e.to_string()))
                .and_then(|b| {
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
                    restored.setup = std::mem::take(&mut s.setup);
                    restored.ai_setup = std::mem::take(&mut s.ai_setup);
                    if !s.remote && !restored.remote {
                        restored.endpoint = s.endpoint.clone();
                    }
                    *s = restored;
                }
                Err(e) => s.error = e,
            }
        }
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
        } else if !paths.is_empty() {
            s.add_files(paths);
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
// One consistent workspace grid; stack at narrow/docked widths.
fn workspace_grid(ui: &mut egui::Ui, mut section: impl FnMut(&mut egui::Ui, usize)) {
    if ui.available_width() >= 720. {
        ui.columns(2, |columns| {
            for (index, column) in columns.iter_mut().enumerate() {
                egui::Frame::group(column.style()).inner_margin(16.).show(column, |ui| {
                    ui.set_min_width((ui.available_width() - 4.).max(0.));
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| section(ui, index));
                });
            }
        });
    } else {
        for index in 0..2 {
            egui::Frame::group(ui.style()).inner_margin(12.).show(ui, |ui| {
                ui.set_min_width((ui.available_width() - 4.).max(0.));
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| section(ui, index));
            });
            ui.add_space(8.);
        }
    }
}

fn audio(app: &mut FilmcraftApp, ui: &mut egui::Ui, s: &mut State) {
    ui.heading("Episode audio");
    ui.add_space(12.);
    let master = rows(&s.project, "clips").into_iter().find(|c| c["camera"] == "master");
    workspace_grid(ui, |ui, column| {
        if column == 0 {
            ui.strong("Recording");
            ui.add_space(12.);
            if let Some(master) = &master {
                ui.heading(string(&s.project, "name"));
                let path = string(master, "path");
                ui.label(Path::new(&path).file_name().unwrap_or_default().to_string_lossy()).on_hover_text(&path);
                ui.label(format!("{:.1} min · {} channels", number(&s.project, "duration") / 60., master["channels"]));
                ui.add_space(16.);
                if button(app, ui, "existing_speakers", "Continue · Speakers") {
                    s.step = 2;
                }
            } else {
                ui.label("Episode name");
                field(app, ui, "name", &mut s.name);
                ui.label("Master audio");
                field(app, ui, "master", &mut s.master);
                picker(app, ui, s);
                ui.add_space(12.);
                ui.add_enabled_ui(!s.master.trim().is_empty(), |ui| {
                    if button(app, ui, "create", "Import & identify speakers") {
                        s.import_audio(true);
                    }
                });
            }
        } else {
            ui.strong("Processing");
            ui.add_space(12.);
            ui.horizontal_wrapped(|ui| {
                for (device, label) in [("auto", "Auto"), ("cpu", "CPU"), ("cuda", "NVIDIA GPU")] {
                    let r = ui.selectable_value(&mut s.compute_device, device.into(), label);
                    app.auto.add_response(&format!("podcast.device.{device}"), &r, label);
                    if r.changed() && s.project.is_object() {
                        s.project["settings"]["compute_device"] = json!(s.compute_device);
                    }
                }
            });
            ui.add_space(16.);
            for (key, label) in [("diarize", "Speakers"), ("transcribe", "Transcript"), ("reactions", "Face analysis")] {
                let ready = s.setup.capabilities["installed_providers"][key] == true;
                ui.horizontal(|ui| {
                    ui.label(label);
                    ui.weak(if ready { "Ready" } else { "Setup needed" });
                });
            }
            ui.add_space(12.);
            if button(app, ui, "audio.models", "Set up models…") {
                s.step = 0;
                s.start("capabilities", |c| c.get("/v1/capabilities"));
            }
        }
    });
}
fn speakers(app: &mut FilmcraftApp, ui: &mut egui::Ui, s: &mut State) {
    if rows(&s.project, "activity").is_empty() && !s.project_id.is_empty() && button(app, ui, "identify_speakers", "Identify speakers") {
        s.job("diarize", json!({}));
    }
    if s.error.contains("Model setup") && button(app, ui, "setup_models", "Set up models") {
        s.step = 0;
        s.start("capabilities", |c| c.get("/v1/capabilities"));
    }
    ui.horizontal_wrapped(|ui| {
        ui.heading("Assign speakers");
        if button(app, ui, "samples", "Load voice samples") {
            let id = s.project_id.clone();
            s.start("samples", move |c| c.get(&format!("/v1/projects/{id}/speaker-samples")));
        }
    });
    ui.add_space(12.);
    let samples = s.samples.clone();
    workspace_grid(ui, |ui, column| {
        for (index, row) in samples.iter().enumerate().filter(|(index, _)| index % 2 == column) {
            let speaker = string(row, "speaker");
            let host = s.project["settings"]["host_speaker"].as_str() == Some(speaker.as_str());
            egui::Frame::group(ui.style()).inner_margin(20.).show(ui, |ui| {
                ui.set_min_width((ui.available_width() - 8.).max(120.));
                ui.heading(format!("Speaker {}", index + 1));
                ui.label(if host {
                    "Host"
                } else if string(&s.project["settings"], "host_speaker").is_empty() {
                    "Not assigned"
                } else {
                    "Guest"
                });
                ui.add_space(12.);
                ui.label(format!("{:.0}–{:.0}s", number(row, "start"), number(row, "end")));
                if button(app, ui, &format!("listen.{speaker}"), "Play sample") {
                    sample(s, number(row, "start"), number(row, "end"));
                }
                let r = ui.selectable_label(host, "Choose as host");
                app.auto.add_response(&format!("podcast.host.{speaker}"), &r, "Choose host");
                if r.clicked() {
                    s.project["settings"]["host_speaker"] = json!(speaker);
                }
            });
            ui.add_space(12.);
        }
    });
    ui.add_space(12.);
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_enabled_ui(!samples.is_empty() && !string(&s.project["settings"], "host_speaker").is_empty(), |ui| {
                if button(app, ui, "lock_roles", "Confirm · Cameras") {
                    let host = string(&s.project["settings"], "host_speaker");
                    match lock_roles(s, &host) {
                        Ok(()) => {
                            s.follow_up = "cameras".into();
                            s.save();
                        }
                        Err(error) => s.error = error,
                    }
                }
            });
        });
    });
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
    ui.heading("Cameras & sync");
    ui.add_space(8.);
    if let Some(master) = rows(&s.project, "clips").iter().find(|c| c["camera"] == "master") {
        ui.label(format!("Episode audio · {}", Path::new(&string(master, "path")).file_name().unwrap_or_default().to_string_lossy()));
    }
    let recordings = rows(&s.project, "clips");
    ui.horizontal_wrapped(|ui| {
        ui.add_enabled_ui(recordings.iter().any(|c| c["camera"] != "master"), |ui| {
            if button(app, ui, "framing.suggest", "Suggest framing") {
                s.job("framing", json!({}));
            }
            if button(app, ui, "framing.preview", "Preview split screen") {
                app.stop();
                s.review_preview(true);
            }
        });
        framing_mode(app, ui, s);
    });
    let duration = number(&s.project, "duration");
    let mut edges = vec![0., duration];
    for c in recordings.iter().filter(|c| c["camera"] != "master") {
        edges.push(number(c, "offset").clamp(0., duration));
        edges.push((number(c, "offset") + number(c, "duration") * number(c, "rate")).clamp(0., duration));
    }
    edges.sort_by(f64::total_cmp);
    edges.dedup();
    let mut cuts = rows(&s.project, "cuts");
    if recordings.iter().any(|c| c["camera"] != "master") && recordings.iter().filter(|c| c["camera"] != "master").all(|c| c["sync_reviewed"] == true) {
        for pair in edges.windows(2) {
            let (start, end) = (pair[0], pair[1]);
            let mid = (start + end) / 2.;
            if end - start > 0.05
                && !recordings
                    .iter()
                    .any(|c| c["camera"] != "master" && number(c, "offset") <= mid && mid < number(c, "offset") + number(c, "duration") * number(c, "rate"))
            {
                let id = format!("gap-{:.0}-{:.0}", start * 1000., end * 1000.);
                let mut approved = cuts.iter().any(|c| c["id"] == id && c["approved"] == true);
                if ui.checkbox(&mut approved, format!("Remove camera gap · {start:.1}–{end:.1}s")).changed() {
                    cuts.retain(|c| c["id"] != id);
                    if approved && !cuts.iter().any(|c| number(c, "start") < end && number(c, "end") > start) {
                        cuts.push(json!({"id":id,"start":start,"end":end,"approved":true}));
                    }
                }
            }
        }
    }
    set_rows(s, "cuts", cuts);
    ui.horizontal_wrapped(|ui| {
        ui.add_enabled_ui(recordings.iter().any(|c| c["camera"] != "master"), |ui| {
            if button(app, ui, "sync", "Sync recordings") {
                s.job("sync", json!({}));
            }
            if button(app, ui, "save_sync", "Save sync") {
                s.save();
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let cameras = rows(&s.project, "clips");
            let ready =
                cameras.iter().any(|c| c["camera"] != "master") && cameras.iter().filter(|c| c["camera"] != "master").all(|c| c["sync_reviewed"] == true);
            ui.add_enabled_ui(ready, |ui| {
                if button(app, ui, "transcribe", "Continue · Transcript") {
                    s.step = 4;
                    s.remember_view();
                }
            });
        });
    });
    egui::CollapsingHeader::new("Add camera files").default_open(!rows(&s.project, "clips").iter().any(|c| c["camera"] != "master")).show(ui, |ui| {
        picker(app, ui, s);
        workspace_grid(ui, |ui, column| {
            if column == 0 {
                ui.strong("Host camera");
                shard_list(app, ui, &mut s.camera_a, "a");
            } else {
                ui.strong("Guest camera");
                shard_list(app, ui, &mut s.camera_b, "b");
            }
        });
        ui.add_enabled_ui(!s.camera_a.is_empty() || !s.camera_b.is_empty(), |ui| {
            if button(app, ui, "attach", "Add recordings") {
                s.job("attach_cameras", json!({"A":s.camera_a,"B":s.camera_b}));
            }
        });
    });
    let mut clips = rows(&s.project, "clips");
    let mut preview = None;
    workspace_grid(ui, |ui, column| {
        let camera = if column == 0 { "A" } else { "B" };
        ui.heading(if column == 0 { "Host camera" } else { "Guest camera" });
        ui.add_space(8.);
        if !clips.iter().any(|c| c["camera"] == camera) {
            ui.weak("Add a recording");
        }
        for clip in clips.iter_mut().filter(|c| c["camera"] == camera) {
            let id = string(clip, "id");
            let sync_id = format!("sync.{id}");
            let original_offset = clip["offset"].clone();
            let original_rate = clip["rate"].clone();
            ui.group(|ui| {
                ui.set_min_width((ui.available_width() - 8.).max(120.));
                let path = string(clip, "path");
                let name = Path::new(&path).file_name().unwrap_or_default().to_string_lossy();
                ui.strong(format!("{} · {name}", if clip["camera"] == "A" { "Host" } else { "Guest" }));
                ui.horizontal_wrapped(|ui| {
                    for (position, label) in [(0., "Preview start"), (0.5, "Preview middle"), (1., "Preview end")] {
                        if button(app, ui, &format!("sync.preview.{id}.{position}"), label) {
                            let first = number(clip, "offset").max(0.);
                            let last = (number(clip, "offset") + number(clip, "duration") * number(clip, "rate")).min(number(&s.project, "duration"));
                            preview = Some((id.clone(), first + (last - first - 12.).max(0.) * position));
                        }
                    }
                    let mut reviewed = clip["sync_reviewed"] == true;
                    let r = ui.checkbox(&mut reviewed, "Sync confirmed");
                    app.auto.add_response(&format!("podcast.sync.reviewed.{id}"), &r, "Sync confirmed");
                    if r.changed() {
                        clip["sync_reviewed"] = json!(reviewed);
                    }
                });
                if let Some(score) = clip["sync_score"].as_f64() {
                    ui.label(format!(
                        "Master interval: {:.2}s – {:.2}s",
                        number(clip, "offset"),
                        number(clip, "offset") + number(clip, "duration") * number(clip, "rate")
                    ));
                    if let Some(residual) = clip["sync_residual_seconds"].as_f64() {
                        ui.small(format!("{} matching anchors · timing-fit error {:.1} ms", clip["sync_anchor_count"].as_u64().unwrap_or(0), residual * 1000.));
                    }
                    ui.weak(format!("Match quality {score:.1}")).on_hover_text("Audio peak-to-noise ratio");
                }
                for warning in clip["warnings"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                    if warning != "sync_review_required" || clip["sync_reviewed"] != true {
                        ui.colored_label(egui::Color32::YELLOW, warning.replace('_', " "));
                    }
                }
                let framing = egui::CollapsingHeader::new("Framing").id_salt(("camera-framing", &id)).show(ui, |ui| camera_framing(app, ui, clip));
                app.auto.add_response(&format!("podcast.framing.open.{id}"), &framing.header_response, "Camera framing");
                let timing = egui::CollapsingHeader::new("Adjust timing").id_salt(("camera-timing", &id)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Offset (s)");
                        numeric(app, ui, clip, "offset", &sync_id);
                        ui.label("Clock drift");
                        numeric(app, ui, clip, "rate", &sync_id);
                    });
                });
                app.auto.add_response(&format!("podcast.timing.open.{id}"), &timing.header_response, "Adjust timing");
                if clip["offset"] != original_offset || clip["rate"] != original_rate {
                    clip["sync_reviewed"] = json!(false);
                    let mut cuts = rows(&s.project, "cuts");
                    cuts.retain(|cut| !string(cut, "id").starts_with("gap-"));
                    set_rows(s, "cuts", cuts);
                }
            });
        }
    });
    set_rows(s, "clips", clips);
    if let Some((clip_id, time)) = preview
        && !s.busy()
    {
        s.review_seek = time;
        if s.review_active
            && s.review_clip.as_ref() == Some(&clip_id)
            && s.review_item == app.session.state.active_sequence.map(|id| id.0)
            && s.project == s.saved_project
            && !s.busy()
        {
            app.seek_program(filmcraft_time::Tick::from_seconds_f64(time));
            if !app.playback.playing {
                app.play(1.);
            }
        } else {
            app.stop();
            s.review_clip = Some(clip_id.clone());
            s.review_autoplay = true;
            s.request_review(false, Some(clip_id));
        }
    }
}
fn review(app: &mut FilmcraftApp, ui: &mut egui::Ui, s: &mut State) {
    ui.heading("Transcript");
    ui.horizontal_wrapped(|ui| {
        let label = if rows(&s.project, "captions").is_empty() { "Generate transcript" } else { "Regenerate transcript…" };
        if button(app, ui, "generate_transcript", label) {
            s.job("transcribe", json!({}));
        }
        ui.collapsing("Processing", |ui| {
            ui.label("Audio per transcription segment (seconds)");
            let mut seconds = s.project["settings"]["transcription_segment_seconds"].as_f64().unwrap_or(28.);
            let r = ui.add(egui::Slider::new(&mut seconds, 8.0..=28.0).step_by(1.));
            app.auto.add_response("podcast.segment_seconds", &r, "Segment seconds");
            if r.changed() {
                s.project["settings"]["transcription_segment_seconds"] = json!(seconds);
            }
        });
    });
    let mut save = false;
    let mut accept = false;
    ui.horizontal_wrapped(|ui| {
        save = button(app, ui, "save_review", "Save");
        if !rows(&s.project, "captions").is_empty() {
            accept = button(app, ui, "accept_transcript", "Accept transcript");
        }
    });
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
    let play_time = app.session.playhead().seconds();
    let playing = s.review_active && app.playback.playing;
    if playing && let Some(index) = captions.iter().position(|c| number(c, "start") <= play_time && play_time < number(c, "end")) {
        s.review_page = index;
    }
    let mut remove = None;
    s.review_page = s.review_page.min(captions.len().saturating_sub(1));
    ui.horizontal(|ui| {
        if button(app, ui, "captions.previous", "← Previous") {
            app.stop();
            s.review_page = s.review_page.saturating_sub(1);
        }
        ui.label(format!("{} / {}", if captions.is_empty() { 0 } else { s.review_page + 1 }, captions.len()));
        if button(app, ui, "captions.next", "Next →") {
            app.stop();
            s.review_page = (s.review_page + 1).min(captions.len().saturating_sub(1));
        }
    });
    if captions.is_empty() {}
    for (i, c) in captions.iter_mut().enumerate().skip(s.review_page).take(1) {
        ui.push_id(i, |ui| {
            ui.group(|ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.strong(string(c, "speaker"));
                    ui.weak(format!("{:.1}–{:.1}s", number(c, "start"), number(c, "end")));
                    if button(app, ui, &format!("caption.play.{i}"), "Play segment") {
                        if let Some(item) = s.review_item {
                            let result = app.session.execute("sequence.open", json!({"item":item})).and_then(|_| {
                                app.session.execute("playhead.set", json!({"time":filmcraft_time::Tick::from_seconds_f64(number(c, "start")).0}))
                            });
                            if let Err(error) = result {
                                s.error = error.to_string();
                            } else {
                                s.review_active = true;
                                app.play(1.);
                            }
                        } else {
                            s.review_seek = number(c, "start");
                            s.review_preview(false);
                        }
                    }
                    ui.collapsing("Timing & speaker", |ui| {
                        let mut speaker = string(c, "speaker");
                        if field(app, ui, &format!("caption.speaker.{i}"), &mut speaker).changed() {
                            c["speaker"] = json!(speaker);
                            c["reviewed"] = json!(false);
                        }
                        numeric(app, ui, c, "start", &format!("caption.{i}"));
                        numeric(app, ui, c, "end", &format!("caption.{i}"));
                        if button(app, ui, &format!("caption.remove.{i}"), "Remove text") {
                            remove = Some(i);
                        }
                    });
                });
                let mut text = string(c, "text");
                let r = egui::ScrollArea::vertical()
                    .id_salt(("caption-text", i))
                    .max_height(76.)
                    .show(ui, |ui| ui.add(egui::TextEdit::multiline(&mut text).desired_rows(3).desired_width(ui.available_width())))
                    .inner;
                app.auto.add_response(&format!("podcast.caption.text.{i}"), &r, "Odia text");
                if r.gained_focus() {
                    app.stop();
                }
                if r.changed() {
                    app.stop();
                    c["text"] = json!(text);
                    c["reviewed"] = json!(false);
                }
                if c["overlap"] == true {
                    ui.label("Overlapping speech");
                }
                if playing && number(c, "start") <= play_time && play_time < number(c, "end") {
                    r.scroll_to_me(Some(egui::Align::Center));
                }
                if button(app, ui, &format!("caption.roman.{i}"), "Type in Latin letters") {
                    app.stop();
                    s.caption_id = string(c, "id");
                    s.target_word_index = None;
                    s.target_word.clear();
                    s.roman.clear();
                    s.replacement.clear();
                    s.spelling_choices.clear();
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
    if !s.caption_id.is_empty() && captions.get(s.review_page).is_some_and(|c| string(c, "id") == s.caption_id) {
        ui.separator();
        ui.strong("Latin → Odia");
        ui.label(if s.target_word.is_empty() { "Replacing the current segment".into() } else { format!("Replacing: {}", s.target_word) });
        ui.horizontal_wrapped(|ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut s.roman).desired_width(260.).hint_text("Type Latin spelling · namaskaara"));
            app.auto.add_response("podcast.roman", &r, "Latin spelling");
            if button(app, ui, "transliterate", "Convert to Odia") {
                s.job("transliterate", json!({"text":s.roman}));
            }
        });
        for choice in s.spelling_choices.clone() {
            if button(app, ui, &format!("spelling.{choice}"), &choice) {
                s.replacement = choice;
            }
        }
        ui.horizontal_wrapped(|ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut s.replacement).desired_width(260.).hint_text("Check the Odia preview"));
            app.auto.add_response("podcast.replacement", &r, "Odia preview");
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
                } else if s.target_word.is_empty() && !s.replacement.trim().is_empty() {
                    c["text"] = json!(s.replacement);
                    c["reviewed"] = json!(false);
                } else {
                    s.error = "Convert Latin text and check the Odia preview before replacing".into();
                }
            }
            if button(app, ui, "close_latin", "Done") {
                s.caption_id.clear();
            }
        });
    }
    set_rows(s, "captions", captions);
    if save {
        s.save();
    }
    if accept {
        for caption in s.project["captions"].as_array_mut().into_iter().flatten() {
            caption["reviewed"] = json!(true);
        }
        if rows(&s.project, "shots").is_empty() {
            s.job("shots", json!({}));
        } else {
            s.step = 5;
            s.remember_view();
            s.save();
        }
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

fn calibrated_smile_threshold(samples: &Value) -> Result<f64, String> {
    let median = |key: &str| -> Result<f64, String> {
        let mut values: Vec<f64> = samples[key]
            .as_array()
            .into_iter()
            .flatten()
            .take(20)
            .filter_map(|row| row["value"].as_f64())
            .filter(|v| v.is_finite() && (0.0..=1.0).contains(v))
            .collect();
        if values.len() < 3 {
            return Err("Mark at least three different neutral frames and three modest-smile frames from the same camera angle.".into());
        }
        values.sort_by(f64::total_cmp);
        let middle = values.len() / 2;
        Ok((values.get(middle).copied().unwrap_or(0.) + values.get((values.len() - 1) / 2).copied().unwrap_or(0.)) / 2.)
    };
    let neutral = median("neutral")?;
    let smile = median("smile")?;
    if smile - neutral < 0.03 {
        return Err("These samples are too similar. Choose clearer modest smiles or more relaxed neutral frames.".into());
    }
    Ok((neutral + smile) / 2.)
}

fn smile_calibration(app: &mut FilmcraftApp, ui: &mut egui::Ui, s: &mut State) {
    let mut threshold =
        s.project["reaction_calibration"]["smile_threshold"].as_f64().or_else(|| s.project["reaction_analysis"]["smile_threshold"].as_f64()).unwrap_or(0.35);
    ui.horizontal_wrapped(|ui| {
        ui.label("Smile");
        let r = ui
            .add(egui::Slider::new(&mut threshold, 0.0..=1.0).fixed_decimals(2))
            .on_hover_text("Expression intensity, not confidence. Apply suggestions to update matching shots.");
        app.auto.add_response("podcast.smile_threshold", &r, "Smile intensity threshold, not confidence");
        if r.changed() {
            s.project["reaction_calibration"]["smile_threshold"] = json!(threshold);
        }
    });
    let calibration_group = ui.collapsing("Smile calibration", |ui| {
        let time = app.session.playhead().seconds();
        let sample = s.project["reaction_analysis"]["measurements"]
            .as_array()
            .and_then(|rows| rows.iter().find(|m| m["detected"] == true && number(m, "time") <= time && time - number(m, "time") < 1.))
            .cloned();
        ui.horizontal_wrapped(|ui| {
            for (key, label) in [("neutral", "Mark neutral frame"), ("smile", "Mark modest smile")] {
                if button(app, ui, &format!("smile_sample.{key}"), label) {
                    if let Some(m) = &sample {
                        let samples = &mut s.project["reaction_calibration"]["smile_samples"][key];
                        if !samples.is_array() {
                            *samples = json!([]);
                        }
                        if let Some(rows) = samples.as_array_mut() {
                            if rows.len() >= 20 {
                                s.error = "Twenty samples are enough. Reset samples to start again.".into();
                            } else if !rows.iter().any(|r| r["time"] == m["time"] && r["clip_id"] == m["clip_id"]) {
                                rows.push(json!({"time":m["time"],"clip_id":m["clip_id"],"value":m["smile"]}));
                            }
                        }
                    } else {
                        s.error = "Choose a measured frame with a clear host face. Run analysis first if no measurements are available.".into();
                    }
                }
            }
            if button(app, ui, "smile_calibrate", "Use calibrated threshold") {
                match calibrated_smile_threshold(&s.project["reaction_calibration"]["smile_samples"]) {
                    Ok(value) => {
                        s.project["reaction_calibration"]["smile_threshold"] = json!(value);
                        s.error.clear();
                    }
                    Err(error) => s.error = error,
                }
            }
            if button(app, ui, "smile_reset", "Reset samples") {
                s.project["reaction_calibration"]["smile_samples"] = json!({});
            }
        });
        let samples = &s.project["reaction_calibration"]["smile_samples"];
        ui.label(format!(
            "Neutral frames: {} · Modest smiles: {}",
            samples["neutral"].as_array().map_or(0, Vec::len),
            samples["smile"].as_array().map_or(0, Vec::len)
        ));
    });
    app.auto.add_response("podcast.smile_calibration", &calibration_group.header_response, "Smile calibration");
}

fn visibility(project: &Value, pending: bool) -> (f64, f64, f64) {
    let host = string(&project["settings"], "host_speaker");
    let camera = &project["settings"]["speaker_camera"][&host];
    let shots = rows(project, "shots");
    let reactions = rows(project, "reactions");
    let cuts = rows(project, "cuts");
    let duration = number(project, "duration").max(0.);
    let mut boundaries = vec![0., duration];
    for item in shots.iter().chain(reactions.iter()).chain(cuts.iter()) {
        boundaries.extend([number(item, "start"), number(item, "end")].into_iter().filter(|t| t.is_finite() && *t >= 0. && *t <= duration));
    }
    boundaries.sort_by(f64::total_cmp);
    boundaries.dedup();
    let (mut host_time, mut guest_time, mut total) = (0., 0., 0.);
    for pair in boundaries.windows(2) {
        let [a, b] = pair else {
            continue;
        };
        let mid = (a + b) / 2.;
        if cuts.iter().any(|c| c["approved"] == true && number(c, "start") <= mid && mid < number(c, "end")) {
            continue;
        }
        let Some(shot) = shots.iter().find(|q| number(q, "start") <= mid && mid < number(q, "end")) else {
            continue;
        };
        let reaction =
            reactions.iter().any(|r| (r["decision"] == "keep" || pending && r["decision"] == "pending") && number(r, "start") <= mid && mid < number(r, "end"));
        let length = b - a;
        total += length;
        if reaction || shot["layout"] == *camera || shot["layout"] == "both" {
            host_time += length;
        }
        if !reaction && shot["layout"] != *camera && shot["layout"] != "black" {
            guest_time += length;
        }
    }
    (host_time, guest_time, total)
}

fn visibility_controls(app: &mut FilmcraftApp, ui: &mut egui::Ui, s: &mut State) {
    if !s.project["reaction_calibration"].is_object() {
        s.project["reaction_calibration"] = json!({});
    }
    ui.horizontal_wrapped(|ui| {
        let mut enabled = s.project["reaction_calibration"]["host_target"].as_f64().is_some();
        let r = ui.checkbox(&mut enabled, "Host target");
        app.auto.add_response("podcast.host_target_enabled", &r, "Use a host visibility target instead of one reaction per interval");
        let mut percent = s.project["reaction_calibration"]["host_target"].as_f64().unwrap_or(0.33) * 100.;
        let slider = ui.add_enabled(enabled, egui::Slider::new(&mut percent, 0.0..=100.0).suffix("%"));
        app.auto.add_response("podcast.host_target", &slider, "Desired host share including speech and split screen");
        if r.changed() || slider.changed() {
            s.project["reaction_calibration"]["host_target"] = if enabled { json!(percent / 100.) } else { Value::Null };
        }
    });
}

fn framing_mode(app: &mut FilmcraftApp, ui: &mut egui::Ui, s: &mut State) {
    let mut mode = s.project["settings"]["split_framing"].as_str().unwrap_or("fill").to_owned();
    for (value, label) in [("fill", "Fill frame"), ("fit", "Fit full image")] {
        let r = ui.selectable_value(&mut mode, value.into(), label);
        app.auto.add_response(&format!("podcast.framing.{value}"), &r, label);
        if r.changed() {
            s.project["settings"]["split_framing"] = json!(mode);
        }
    }
}
fn camera_framing(app: &mut FilmcraftApp, ui: &mut egui::Ui, clip: &mut Value) {
    let id = string(clip, "id");
    for (key, label) in [("frame_x", "Horizontal"), ("frame_y", "Vertical")] {
        let mut value = clip[key].as_f64().unwrap_or(0.5);
        let r = ui.add(egui::Slider::new(&mut value, 0.0..=1.0).text(label).fixed_decimals(2));
        app.auto.add_response(&format!("podcast.framing.{id}.{key}"), &r, label);
        if r.changed() {
            clip[key] = json!(value);
            clip["framing_reviewed"] = json!(false);
        }
    }
    let mut locked = clip["framing_reviewed"] == true;
    let r = ui.checkbox(&mut locked, "Framing confirmed");
    app.auto.add_response(&format!("podcast.framing.{id}.confirm"), &r, "Framing confirmed");
    if r.changed() {
        clip["framing_reviewed"] = json!(locked);
    }
}
fn overlap_controls(app: &mut FilmcraftApp, ui: &mut egui::Ui, s: &mut State) {
    let framing = ui.collapsing("Picture framing", |ui| {
        ui.horizontal_wrapped(|ui| framing_mode(app, ui, s));
        if button(app, ui, "framing.suggest", "Suggest framing") {
            s.job("framing", json!({}));
        }
        let mut clips = rows(&s.project, "clips");
        for clip in clips.iter_mut().filter(|c| c["camera"] != "master") {
            let path = string(clip, "path");
            let id = string(clip, "id");
            let camera = egui::CollapsingHeader::new(format!(
                "{} · {}",
                if clip["camera"] == "A" { "Host" } else { "Guest" },
                Path::new(&path).file_name().unwrap_or_default().to_string_lossy()
            ))
            .id_salt(("reaction-framing", &id))
            .show(ui, |ui| camera_framing(app, ui, clip));
            app.auto.add_response(&format!("podcast.framing.open.{id}"), &camera.header_response, "Camera framing");
        }
        set_rows(s, "clips", clips);
    });
    app.auto.add_response("podcast.framing.controls", &framing.header_response, "Picture framing");
    let group = egui::CollapsingHeader::new("Split screen").default_open(true).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            for (key, label) in [("overlap_before", "Before (s)"), ("overlap_after", "After (s)"), ("meaningful_overlap", "Ignore below (s)")] {
                let fallback = if key == "meaningful_overlap" { 0. } else { s.project["settings"]["overlap_padding"].as_f64().unwrap_or(1.) };
                let mut value = s.project["settings"][key].as_f64().unwrap_or(fallback);
                ui.label(label);
                let r = ui.add(egui::DragValue::new(&mut value).range(0.0..=5.0).speed(0.05));
                app.auto.add_response(&format!("podcast.{key}"), &r, label);
                if r.changed() {
                    s.project["settings"][key] = json!(value);
                }
            }
        });
        if button(app, ui, "overlap_all", "All interruptions · ±1s") {
            s.project["settings"]["meaningful_overlap"] = json!(0.);
            s.project["settings"]["overlap_before"] = json!(1.);
            s.project["settings"]["overlap_after"] = json!(1.);
        }
    });
    app.auto.add_response("podcast.overlap_controls", &group.header_response, "Adjust split screen buffers");
}

fn reference_choices(app: &mut FilmcraftApp, ui: &mut egui::Ui, s: &mut State, hosts: &[Value]) {
    let available: Vec<_> =
        hosts.iter().take(100).filter(|clip| s.reference_candidates[string(clip, "id")].as_array().is_some_and(|v| !v.is_empty())).collect();
    if available.is_empty() {
        return;
    }
    if !available.iter().any(|clip| string(clip, "id") == s.reference_candidate_clip) {
        let middle = number(&s.project, "duration") / 2.;
        if let Some(clip) = available.iter().min_by(|a, b| {
            let distance = |c: &&Value| (number(c, "offset") + number(c, "duration") * number(c, "rate") / 2. - middle).abs();
            distance(a).total_cmp(&distance(b))
        }) {
            s.reference_candidate_clip = string(clip, "id");
        }
    }
    if available.len() > 1 {
        let selector = egui::ComboBox::from_id_salt("reference-recording").selected_text("Recording").show_ui(ui, |ui| {
            for (index, clip) in available.iter().enumerate() {
                let id = string(clip, "id");
                let response = ui.selectable_value(
                    &mut s.reference_candidate_clip,
                    id,
                    format!("{} · {:.0}–{:.0}s", index + 1, number(clip, "offset"), number(clip, "offset") + number(clip, "duration") * number(clip, "rate")),
                );
                app.auto.add_response(&format!("podcast.reference_recording.{index}"), &response, "Reference recording");
            }
        });
        app.auto.add_response("podcast.reference_recording", &selector.response, "Choose reference recording");
    }
    for clip in available.into_iter().filter(|clip| string(clip, "id") == s.reference_candidate_clip.clone()).collect::<Vec<_>>() {
        let id = string(clip, "id");
        let candidates = s.reference_candidates[&id].as_array().cloned().unwrap_or_default();
        if candidates.is_empty() {
            continue;
        }
        ui.horizontal_wrapped(|ui| {
            for (index, candidate) in candidates.iter().take(5).enumerate() {
                let Some(time) = candidate["time"].as_f64().filter(|t| t.is_finite() && *t >= 0. && *t < number(clip, "duration")) else {
                    continue;
                };
                let key = format!("{id}-{index}");
                if !s.reference_thumbnails.contains_key(&key) {
                    let bytes: Vec<u8> = candidate["thumbnail"]
                        .as_array()
                        .filter(|v| v.len() <= 100_000)
                        .map(|v| v.iter().filter_map(|b| b.as_u64().and_then(|n| u8::try_from(n).ok())).collect())
                        .unwrap_or_default();
                    if let Ok(mut reader) = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format() {
                        let mut limits = image::Limits::default();
                        limits.max_image_width = Some(512);
                        limits.max_image_height = Some(512);
                        limits.max_alloc = Some(4 * 1024 * 1024);
                        reader.limits(limits);
                        if let Ok(image) = reader.decode() {
                            let image = image.to_rgba8();
                            let pixels = egui::ColorImage::from_rgba_unmultiplied([image.width() as usize, image.height() as usize], image.as_raw());
                            s.reference_thumbnails.insert(key.clone(), ui.ctx().load_texture(&key, pixels, Default::default()));
                        }
                    }
                }
                ui.vertical(|ui| {
                    let selected = s.project["reaction_calibration"]["reference_samples"][&id]
                        .as_array()
                        .is_some_and(|samples| samples.iter().any(|v| v.as_f64().is_some_and(|t| (t - time).abs() < 0.01)));
                    let response = if let Some(texture) = s.reference_thumbnails.get(&key) {
                        ui.add(egui::Button::image(egui::Image::new((texture.id(), egui::vec2(144., 81.)))).selected(selected))
                    } else {
                        ui.selectable_label(selected, format!("Frame {}", index + 1))
                    };
                    app.auto.add_response(&format!("podcast.reference_candidate.{id}.{index}"), &response, "Use this look reference");
                    if response.on_hover_text("Select a frame looking toward the guest").clicked() {
                        let mut samples = s.project["reaction_calibration"]["reference_samples"][&id].as_array().cloned().unwrap_or_default();
                        if selected {
                            samples.retain(|v| v.as_f64().is_none_or(|t| (t - time).abs() >= 0.01));
                        } else if samples.len() < 8 {
                            samples.push(json!(time));
                        }
                        if let Some(first) = samples.first().and_then(Value::as_f64) {
                            s.references.insert(id.clone(), first);
                        } else {
                            s.references.remove(&id);
                        }
                        s.project["reaction_calibration"]["reference_samples"][&id] = json!(samples);
                        app.stop();
                        app.seek_program(filmcraft_time::Tick::from_seconds_f64(number(clip, "offset") + time * number(clip, "rate")));
                    }
                    ui.small(format!("{}{:.1}s", if selected { "✓ " } else { "" }, number(clip, "offset") + time * number(clip, "rate")));
                });
            }
        });
    }
}

fn reactions(app: &mut FilmcraftApp, ui: &mut egui::Ui, s: &mut State) {
    ui.set_min_width(ui.available_width());
    let cameras = rows(&s.project, "clips");
    let cameras: Vec<_> = cameras.iter().filter(|c| c["camera"] == "A" || c["camera"] == "B").collect();
    let sync_ready = !cameras.is_empty() && cameras.iter().all(|c| c["sync_reviewed"] == true);
    ui.horizontal(|ui| {
        ui.strong("Reactions");
        ui.add_enabled_ui(!s.busy(), |ui| {
            if button(app, ui, "reactions.check_sync", if sync_ready { "Check sync" } else { "Check sync · required" }) {
                app.stop();
                s.review_seek = app.session.playhead().seconds();
                s.step = 3;
                s.review_preview(false);
            }
        });
        let pending = rows(&s.project, "reactions").iter().filter(|r| r["decision"] == "pending").count();
        ui.weak(format!("{pending} to review"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_enabled_ui(pending == 0 && sync_ready && !s.busy(), |ui| {
                if s.remote && button(app, ui, "open_editor", "Open editor") {
                    s.job("proxies", json!({}));
                }
                if !s.remote && button(app, ui, "open_originals", "Continue · Editor & export") {
                    let id = s.project_id.clone();
                    let project = s.project.clone();
                    s.start("assemble", move |c| {
                        let saved = c.request(reqwest::Method::PUT, &format!("/v1/projects/{id}"), Some(&project))?;
                        match c.get(&format!("/v1/projects/{id}/filmcraft")) {
                            Ok(plan) => Ok(json!({"plan":plan,"project":saved,"storage":c.get(&format!("/v1/projects/{id}/storage"))?})),
                            Err(error) => Ok(json!({"error":error,"project":saved})),
                        }
                    });
                }
            });
        });
    });
    ui.separator();
    if s.recalibrating || !s.project["reaction_analysis"]["candidates"].is_array() {
        egui::ScrollArea::vertical().id_salt("reaction-reference-setup").show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let manual = ui.collapsing("Manual shots", |ui| reaction_review(app, ui, s));
            app.auto.add_response("podcast.manual_shots", &manual.header_response, "Edit host shots without running detection");
            reaction_setup(app, ui, s);
        });
        return;
    }
    let height = ui.available_height().max(100.);
    let width = ui.available_width();
    let inspector = (width * 0.32).clamp(280., 430.).min(width * 0.5);
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(egui::vec2((width - inspector - 20.).max(100.), height), egui::Layout::top_down(egui::Align::Min), |ui| {
            egui::ScrollArea::vertical().id_salt("reaction-review").max_height(height).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                reaction_review(app, ui, s);
            });
        });
        ui.separator();
        ui.allocate_ui_with_layout(egui::vec2(inspector, height), egui::Layout::top_down(egui::Align::Min), |ui| {
            egui::ScrollArea::vertical().id_salt("reaction-inspector").max_height(height).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.strong("Detection & timing");
                reaction_setup(app, ui, s);
            });
        });
    });
}

fn reaction_setup(app: &mut FilmcraftApp, ui: &mut egui::Ui, s: &mut State) {
    let host = string(&s.project["settings"], "host_speaker");
    let camera = s.project["settings"]["speaker_camera"][&host].clone();
    let ready = s.setup.capabilities["installed_providers"]["reactions"] == true;
    let analyzed = !s.recalibrating && s.project["reaction_analysis"]["candidates"].is_array();
    if analyzed {
        s.tolerance = s.project["reaction_analysis"]["tolerance"].as_f64().unwrap_or(s.tolerance);
    }
    let calibration_before = s.project["reaction_calibration"].clone();
    if !ready {
        ui.horizontal_wrapped(|ui| {
            ui.label("Reaction model missing.");
            if button(app, ui, "reaction_models", "Install reactions") {
                s.setup.start("Setup-vision.ps1", "-Mode", "cpu");
            }
        });
    }
    if analyzed {
        ui.horizontal_wrapped(|ui| {
            if button(app, ui, "reaction_reselect", "Refresh suggestions") {
                let threshold = s.project["reaction_calibration"]["smile_threshold"].as_f64().unwrap_or(0.35);
                s.job("reaction_select", json!({"interval":s.interval,"spacing":s.spacing,"smile_threshold":threshold,"host_target":s.project["reaction_calibration"]["host_target"]}));
            }
            let r = ui.checkbox(&mut s.show_detection, "Signals");
            app.auto.add_response("podcast.show_detection", &r, "Show measured host eye and head cues");
            if button(app, ui, "reaction_recalibrate", "Reference…") {
                s.recalibrating = true;
                s.status = "Existing reactions retained. Adjust settings, then explicitly regenerate analysis.".into();
            }
        });
    } else {
        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.add_enabled_ui(ready, |ui| {
                    if button(app, ui, "suggest_references", "Find 5 frames") {
                        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().subsec_nanos();
                        s.job("reference_suggestions", json!({"reference_seed":seed}));
                    }
                });
                if button(app, ui, "reference_image", "Upload image…") {
                    if let Some(path) = app.hooks.pick_files.as_mut().and_then(|f| f(&["png", "jpg", "jpeg"]).into_iter().next()) {
                        s.reference_image = path;
                    }
                }
                if !s.reference_image.is_empty() {
                    ui.label("Screenshot reference selected");
                    if button(app, ui, "reference_image_clear", "Use video references") { s.reference_image.clear(); }
                }
            });
            let hosts: Vec<Value> = rows(&s.project, "clips").into_iter().filter(|c| c["camera"] == camera).collect();
            ui.label(format!("{} / {} recordings ready", hosts.iter().filter(|c| s.references.contains_key(&string(c,"id"))).count(), hosts.len()));
            reference_choices(app, ui, s, &hosts);
            let now = app.session.playhead().seconds();
            if let Some(clip) = hosts.iter().find(|c| number(c,"offset") <= now && now < number(c,"offset")+number(c,"duration")*number(c,"rate")) {
                let id = string(clip,"id");
                ui.horizontal_wrapped(|ui| {
                    if s.review_active && button(app, ui, "reference_current", "Use current frame") {
                        let time = (now-number(clip,"offset"))/number(clip,"rate").max(0.98);
                        s.references.insert(id.clone(), time);
                        if !s.project["reaction_calibration"].is_object() { s.project["reaction_calibration"] = json!({}); }
                        if !s.project["reaction_calibration"]["reference_samples"].is_object() { s.project["reaction_calibration"]["reference_samples"] = json!({}); }
                        let mut samples = s.project["reaction_calibration"]["reference_samples"][&id].as_array().cloned().unwrap_or_default();
                        if samples.len() < 8 && !samples.iter().any(|v| v.as_f64().is_some_and(|t| (t-time).abs() < 0.25)) { samples.push(json!(time)); }
                        s.project["reaction_calibration"]["reference_samples"][&id] = json!(samples);
                    }
                    if let Some(time) = s.references.get(&id).copied() {
                        let samples = s.project["reaction_calibration"]["reference_samples"][&id].as_array().cloned().unwrap_or_default();
                        ui.label(format!("{} examples", samples.len().max(1)));
                        if button(app, ui, "reference_reset", "Clear examples") {
                            s.project["reaction_calibration"]["reference_samples"][&id] = json!([]);
                            s.references.remove(&id);
                        }
                        ui.label(format!("Latest: {:.1}s", number(clip,"offset")+time*number(clip,"rate")));
                        if button(app, ui, "reference_view", "View reference") { app.stop(); app.seek_program(filmcraft_time::Tick::from_seconds_f64(number(clip,"offset")+time*number(clip,"rate"))); }
                    }
                });
            }
            ui.add_enabled_ui(ready && (!s.reference_image.is_empty() || hosts.iter().all(|c| s.references.contains_key(&string(c, "id")))), |ui| {
                if button(app, ui, "analyze_reactions", "Find smiles & nods") {
                    let threshold = s.project["reaction_calibration"]["smile_threshold"].as_f64().unwrap_or(0.35);
                    s.job("reactions", json!({"references":s.references,"reference_samples":s.project["reaction_calibration"]["reference_samples"].as_object().cloned().unwrap_or_default(),"reference_image":s.reference_image,"tolerance":s.tolerance,"interval":s.interval,"spacing":s.spacing,"smile_threshold":threshold,"host_target":s.project["reaction_calibration"]["host_target"]}));
                }
            });
        });
    }
    let has_reference = !s.reference_image.is_empty() || !s.references.is_empty();
    if has_reference {
        ui.separator();
        ui.strong("Selection");
        visibility_controls(app, ui, s);
        ui.horizontal_wrapped(|ui| {
            ui.label("Gap · s");
            let r = ui.add(egui::DragValue::new(&mut s.spacing).range(0.0..=60.0).speed(0.5));
            app.auto.add_response("podcast.reaction_spacing", &r, "Minimum gap between reaction inserts");
            if s.project["reaction_calibration"]["host_target"].is_null() {
                ui.label("Interval · s");
                let r = ui.add(egui::DragValue::new(&mut s.interval).range(15.0..=180.0).speed(1.0));
                app.auto.add_response("podcast.reaction_interval", &r, "One reaction per interval");
            }
        });
        let extra = ui.collapsing("Face signals", |ui| {
            ui.add_enabled_ui(!analyzed, |ui| {
                let label = if s.setup.capabilities["reaction_provider"] == "mediapipe-l2cs" { "Gaze tolerance °" } else { "Eye tolerance (%)" };
                ui.label(label);
                let r = ui.add(egui::Slider::new(&mut s.tolerance, 3.0..=30.0));
                app.auto.add_response("podcast.gaze_tolerance", &r, label);
            });
            smile_calibration(app, ui, s);
        });
        app.auto.add_response("podcast.reaction_settings", &extra.header_response, "Face signal settings");
    }
    overlap_controls(app, ui, s);
    let mut calibration = s.project["reaction_calibration"].clone();
    if !calibration.is_object() {
        calibration = json!({});
    }
    calibration["references"] = json!(s.references);
    calibration["reference_image"] = json!(s.reference_image);
    calibration["tolerance"] = json!(s.tolerance);
    calibration["spacing"] = json!(s.spacing);
    if !analyzed && calibration != calibration_before {
        s.project["reaction_calibration"] = calibration;
    }
}

fn reaction_review(app: &mut FilmcraftApp, ui: &mut egui::Ui, s: &mut State) {
    let host = string(&s.project["settings"], "host_speaker");
    let camera = s.project["settings"]["speaker_camera"][&host].clone();
    let before_reactions = s.project["reactions"].clone();
    let before_decisions = s.project["reaction_analysis"]["decisions"].clone();
    let mut undoing = false;
    if s.reaction_undo.as_ref().is_some_and(|(id, _, _)| id == &s.project_id) && button(app, ui, "undo_reaction", "Undo") {
        if let Some((_, previous, decisions)) = s.reaction_undo.take() {
            s.project["reactions"] = previous;
            if s.project["reaction_analysis"].is_object() {
                s.project["reaction_analysis"]["decisions"] = decisions;
            }
            undoing = true;
        }
    }
    let mut reactions = rows(&s.project, "reactions");
    let mut insert_host = false;
    ui.horizontal_wrapped(|ui| {
        let mut duration = s.project["reaction_calibration"]["manual_duration"].as_f64().unwrap_or(3.);
        ui.label("Insert · s");
        let r = ui.add(egui::DragValue::new(&mut duration).range(0.1..=60.).speed(0.1));
        app.auto.add_response("podcast.manual_duration", &r, "Manual host insert duration");
        if r.changed() {
            s.project["reaction_calibration"]["manual_duration"] = json!(duration);
        }
        if button(app, ui, "manual_in", "In") {
            s.project["reaction_calibration"]["manual_in"] = json!(app.session.playhead().seconds());
        }
        if button(app, ui, "manual_out", "Out") {
            let start = s.project["reaction_calibration"]["manual_in"].as_f64().unwrap_or(app.session.playhead().seconds());
            let length = app.session.playhead().seconds() - start;
            if length > 0. && length <= 60. {
                s.project["reaction_calibration"]["manual_duration"] = json!(length);
                app.stop();
                app.seek_program(filmcraft_time::Tick::from_seconds_f64(start));
            } else {
                s.error = "Mark an end after the start, within 60 seconds.".into();
            }
        }
        insert_host = s.review_active && button(app, ui, "manual_reaction", "Insert host");
    });
    if insert_host {
        app.stop();
        let start = app.session.playhead().seconds();
        let clip = rows(&s.project, "clips")
            .into_iter()
            .find(|c| c["camera"] == camera && number(c, "offset") <= start && start < number(c, "offset") + number(c, "duration") * number(c, "rate"));
        let shot = rows(&s.project, "shots")
            .into_iter()
            .find(|q| number(q, "start") <= start && start < number(q, "end") && q["layout"] != camera && q["layout"] != "both");
        if let (Some(clip), Some(shot)) = (clip, shot) {
            let end = (start + s.project["reaction_calibration"]["manual_duration"].as_f64().unwrap_or(3.).clamp(0.1, 60.))
                .min(number(&shot, "end"))
                .min(number(&clip, "offset") + number(&clip, "duration") * number(&clip, "rate"));
            if !reactions.iter().any(|r| r["decision"] != "reject" && number(r, "start") < end && number(r, "end") > start) {
                let id = format!("manual-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos());
                reactions.push(json!({"id":id,"clip_id":clip["id"],"start":start,"end":end,"score":1.0,"cues":["manual"],"decision":"keep"}));
            } else {
                s.error = "A reaction already covers this interval".into();
            }
        } else {
            s.error = "Choose guest-only speech where the host camera is available".into();
        }
    }
    let mut remove_reaction = None;
    let current = visibility(&s.project, false);
    let proposed = visibility(&s.project, true);
    ui.horizontal_wrapped(|ui| {
        ui.label(format!(
            "Host {:.1}m · {:.0}%   Guest {:.1}m · {:.0}%",
            current.0 / 60.,
            100. * current.0 / current.2.max(1.),
            current.1 / 60.,
            100. * current.1 / current.2.max(1.)
        ))
        .on_hover_text("Current edit. Split screen counts toward both speakers.");
        if (current.0 - proposed.0).abs() > 0.1 {
            ui.weak(format!(
                "Proposed: host {:.1}m · {:.0}% / guest {:.1}m · {:.0}%",
                proposed.0 / 60.,
                100. * proposed.0 / proposed.2.max(1.),
                proposed.1 / 60.,
                100. * proposed.1 / proposed.2.max(1.)
            ));
        }
    });
    ui.horizontal_wrapped(|ui| {
        let now = app.session.playhead().seconds();
        for (id, label, next) in [("previous_reaction", "Previous", false), ("next_reaction", "Next", true)] {
            if button(app, ui, id, label) {
                let times = reactions.iter().filter(|r| r["decision"] != "reject").map(|r| number(r, "start"));
                let target =
                    if next { times.filter(|t| *t > now + 0.1).min_by(f64::total_cmp) } else { times.filter(|t| *t < now - 0.1).max_by(f64::total_cmp) };
                if let Some(time) = target {
                    app.stop();
                    app.seek_program(filmcraft_time::Tick::from_seconds_f64(time));
                }
            }
        }
        if button(app, ui, "keep_all", "Keep pending") {
            for r in &mut reactions {
                if r["decision"] == "pending" {
                    r["decision"] = json!("keep");
                }
            }
        }
        if button(app, ui, "reject_all", "Remove pending") {
            for r in &mut reactions {
                if r["decision"] == "pending" {
                    r["decision"] = json!("reject");
                }
            }
        }
    });
    if reactions.is_empty() {
        ui.separator();
        let target = s.project["reaction_calibration"]["host_target"].as_f64();
        if target.is_some_and(|v| current.0 / current.2.max(1.) >= v) {
            ui.strong("Host target reached by speech & split screen");
        } else {
            ui.strong("No reaction shots selected");
        }
        let count = s.project["reaction_analysis"]["candidates"].as_array().map_or(0, Vec::len);
        ui.weak(format!("{count} measured spans · adjust selection, then Refresh suggestions"));
    }
    let mut previous_interval = None;
    for (i, r) in reactions.iter_mut().enumerate() {
        let interval = (number(r, "start") / s.interval.max(1.)).floor() as u64;
        if previous_interval != Some(interval) {
            ui.separator();
            ui.strong(format!("{:.0}–{:.0}s", interval as f64 * s.interval, interval.saturating_add(1) as f64 * s.interval));
            previous_interval = Some(interval);
        }
        ui.horizontal_wrapped(|ui| {
            let cues = r["cues"]
                .as_array()
                .map(|values| values.iter().filter_map(Value::as_str).filter(|c| *c != "reference look").collect::<Vec<_>>().join(" · "))
                .unwrap_or_default();
            let label = format!("{:.1}–{:.1}s · {cues}", number(r, "start"), number(r, "end"));
            if button(app, ui, &format!("reaction.preview.{i}"), &label) {
                app.stop();
                s.review_seek = number(r, "start");
                app.seek_program(filmcraft_time::Tick::from_seconds_f64(s.review_seek));
                if !s.preview_suggestions {
                    s.review_reaction = Some(string(r, "id"));
                    s.request_review(true, None);
                }
            }
            for (decision, label) in [("keep", "Keep"), ("reject", "Remove")] {
                let response = ui.selectable_label(r["decision"] == decision, label);
                app.auto.add_response(&format!("podcast.reaction.{i}.{decision}"), &response, label);
                if response.clicked() {
                    r["decision"] = json!(decision);
                }
            }
            let trim = ui.menu_button("Trim", |ui| {
                numeric(app, ui, r, "start", &format!("reaction.{i}"));
                numeric(app, ui, r, "end", &format!("reaction.{i}"));
                if button(app, ui, &format!("reaction.delete.{i}"), "Delete") {
                    remove_reaction = Some(i);
                }
            });
            app.auto.add_response(&format!("podcast.reaction.trim.{i}"), &trim.response, "Trim reaction");
            app.auto.add(&format!("podcast.reaction.{i}"), ui.min_rect(), "Reaction suggestion");
        });
    }
    if let Some(i) = remove_reaction {
        if let Some(r) = reactions.get(i) {
            if !s.project["reaction_analysis"].is_object() {
                s.project["reaction_analysis"] = json!({});
            }
            if !s.project["reaction_analysis"]["decisions"].is_object() {
                s.project["reaction_analysis"]["decisions"] = json!({});
            }
            let key = format!("{}:{}:{}", string(r, "clip_id"), number(r, "start"), number(r, "end"));
            s.project["reaction_analysis"]["decisions"][key] = json!("reject");
        }
        if i < reactions.len() {
            reactions.remove(i);
        }
    }
    set_rows(s, "reactions", reactions);
    if !undoing && before_reactions != s.project["reactions"] {
        s.reaction_undo = Some((s.project_id.clone(), before_reactions, before_decisions));
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
    fn server_once(status: &str, body: &str) -> (String, std::thread::JoinHandle<String>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let reply = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
            let mut request = [0; 8192];
            let size = stream.read(&mut request).unwrap();
            stream.write_all(reply.as_bytes()).unwrap();
            String::from_utf8_lossy(&request[..size]).into_owned()
        });
        (address, worker)
    }

    #[test]
    fn visibility_counts_split_screen_pending_and_removed_ranges() {
        let p = serde_json::json!({"duration":60.,"settings":{"host_speaker":"h","speaker_camera":{"h":"A"}},
            "shots":[{"start":0.,"end":10.,"layout":"A"},{"start":10.,"end":20.,"layout":"both"},{"start":20.,"end":60.,"layout":"B"}],
            "reactions":[{"start":30.,"end":35.,"decision":"keep"},{"start":40.,"end":45.,"decision":"pending"}],
            "cuts":[{"start":50.,"end":60.,"approved":true}]});
        assert_eq!(super::visibility(&p, false), (25., 35., 50.));
        assert_eq!(super::visibility(&p, true), (30., 30., 50.));
    }

    #[test]
    fn native_review_uses_program_transport_and_three_views() {
        use super::*;
        let root = std::env::temp_dir().join(format!("youcraft-review-ui-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let wav = root.join("master.wav");
        let size = 4 * 48000 * 2u32;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + size).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&48000u32.to_le_bytes());
        bytes.extend_from_slice(&96000u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&size.to_le_bytes());
        bytes.resize(44 + size as usize, 0);
        for i in 0..(size as usize / 2) {
            let sample = (12000. * (i as f64 * 440. * std::f64::consts::TAU / 48000.).sin()) as i16;
            bytes[44 + i * 2..46 + i * 2].copy_from_slice(&sample.to_le_bytes());
        }
        std::fs::write(&wav, bytes).unwrap();
        let host = root.join("host.png");
        let guest = root.join("guest.png");
        image::RgbImage::from_pixel(320, 180, image::Rgb([210, 90, 50])).save(&host).unwrap();
        image::RgbImage::from_pixel(320, 180, image::Rgb([50, 110, 210])).save(&guest).unwrap();
        let (host, guest) = if let Some(media) = std::env::var_os("YOUCRAFT_UI_TEST_MEDIA") {
            let media = PathBuf::from(media);
            (media.join("camera-A.mp4"), media.join("camera-B.mov"))
        } else {
            (host, guest)
        };
        let plan = json!({"version":1,"review":true,"review_edit":true,"name":"Reaction review","master":"master",
            "assets":[{"id":"master","path":wav},{"id":"host","path":host},{"id":"guest","path":guest}],
            "clips":[{"start":0,"end":4*254_016_000_000_i64,"video":[
                {"asset":"host","source_in":0,"speed":1.0,"side":"review-bottom-A"},
                {"asset":"guest","source_in":0,"speed":1.0,"side":"review-bottom-B"},
                {"asset":"host","source_in":0,"speed":1.0,"side":"review-top-full"}]}],
            "transcript":{"language":"or","source":"test","speakers":[],"words":[]}});
        let project = json!({"id":"review-test","name":"Review fixture","duration":4,"revision":1,
            "settings":{"host_speaker":"host","speaker_camera":{"host":"A","guest":"B"},"roles_locked":true},
            "clips":[{"id":"master","camera":"master","path":wav,"duration":4},
                     {"id":"host","camera":"A","path":host,"duration":4,"rate":1,"offset":0,"sync_reviewed":true},
                     {"id":"guest","camera":"B","path":guest,"duration":4,"rate":1,"offset":0,"sync_reviewed":true}],
            "activity":[],"shots":[],"reactions":[],"captions":[{"id":"caption","start":0,"end":4,"speaker":"host","text":"ନମସ୍କାର","reviewed":true}]});
        let mut app = FilmcraftApp::new(filmcraft_engine::Session::default());
        app.set_workspace("Podcast");
        let mut state = State {
            project_id: "review-test".into(),
            cache: root.to_string_lossy().into(),
            step: 5,
            projects_open: false,
            action: "review_assemble".into(),
            ..Default::default()
        };
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Ok(json!({"project":project,"plan":plan}))).unwrap();
        state.pending = Some(rx);
        poll(&mut app, &mut state);
        assert!(state.error.is_empty(), "{}", state.error);
        assert!(state.review_active);
        // Regression: replacing a playing review must finish automation on the OLD strips.
        app.play(1.);
        app.end_preroll(0.);
        assert!(app.session.mixrec.active());
        let old_sequence = app.session.state.active_sequence;
        state.action = "review_assemble".into();
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Ok(json!({"project":project,"plan":plan}))).unwrap();
        state.pending = Some(rx);
        poll(&mut app, &mut state);
        assert!(state.error.is_empty(), "Rebuild failed: {}", state.error);
        assert_ne!(old_sequence, app.session.state.active_sequence);
        assert!(!app.session.mixrec.active());
        app.play(1.);
        app.end_preroll(1.);
        for second in [2., 0.4, 3., 1.] {
            app.seek_program(filmcraft_time::Tick::from_seconds_f64(second));
            assert!(app.playback.playing, "Seeking must preserve Play");
            app.end_preroll(second + 2.);
            let mut mix = crate::playback_mix(&app.session, app.session.state.active_sequence.unwrap(), 48000);
            let mut samples = vec![0.; 4800];
            mix((second * 48000.) as i64, &mut samples, 2);
            assert!(samples.iter().any(|x| x.abs() > 0.1), "Master audio silent after seek");
        }
        app.seek_program(filmcraft_time::Tick::from_seconds_f64(100.));
        assert!(!app.playback.playing, "Seeking beyond the end must stop, not restart at zero");
        assert_eq!(app.session.playhead(), app.session.active_sequence().unwrap().duration());
        app.seek_program(filmcraft_time::Tick::ZERO);
        assert!(!app.playback.playing, "Seeking while paused must remain paused");
        app.podcast = state;
        let sequence = app.session.state.active_sequence.unwrap();
        let mut mix = crate::playback_mix(&app.session, sequence, 48000);
        let mut output = vec![0.0; 9600];
        mix(0, &mut output, 2);
        assert!(output.iter().any(|sample| sample.abs() > 0.1), "Review must mix non-silent master audio");
        app.play(1.);
        assert!(app.playback.playing);
        assert!(!app.source_playback.clock.playing);
        app.stop();
        let screenshots = std::env::var_os("FILMCRAFT_UI_SNAPSHOT_DIR").map(PathBuf::from);
        let mut builder = egui_kittest::Harness::builder().with_size(egui::vec2(1366., 768.)).with_max_steps(10000);
        if screenshots.is_some() {
            builder = builder.wgpu();
        }
        let mut harness = builder.build_eframe(move |_| app);
        for _ in 0..20 {
            harness.step();
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(harness.state().auto.find("program.scrubBar").is_some());
        let picture = harness.state().auto.find("program.picture").expect("Visible review video");
        assert!(picture.rect[2] >= 700. && picture.rect[3] >= 200., "Review collapsed: {:?}", picture.rect);

        if let Some(directory) = &screenshots {
            std::fs::create_dir_all(directory).unwrap();
            harness.render().unwrap().save(directory.join("podcast-native-review.png")).unwrap();
        }
        // Opening utility dialogs must never steal height from the playback area.
        harness.state_mut().podcast.setup.ai_open = true;
        harness.state_mut().podcast.episodes = vec![json!({"id":"review-test","name":"Review fixture"})];
        for _ in 0..3 {
            harness.step();
        }
        let with_dialogs = harness.state().auto.find("program.picture").unwrap().rect;
        assert!(with_dialogs[3] >= 200., "Utility dialogs collapsed the player");
        harness.state_mut().podcast.setup.ai_open = false;
        harness.state_mut().podcast.episodes.clear();
        let mut camera_plan = plan.clone();
        camera_plan["review_edit"] = json!(false);
        camera_plan["clips"][0]["video"] = json!([
            {"asset":"host","source_in":0,"speed":1.0,"side":"A"},
            {"asset":"guest","source_in":0,"speed":1.0,"side":"B"}]);
        harness.state_mut().session.execute("podcast.assemble", camera_plan).unwrap();
        for step in [3, 4] {
            harness.state_mut().podcast.step = step;
            for _ in 0..20 {
                harness.step();
                std::thread::sleep(Duration::from_millis(20));
            }
            let picture = harness.state().auto.find("program.picture").unwrap().rect;
            assert!(picture[2] >= 600. && picture[3] >= 180., "Camera review collapsed: {picture:?}");
            if let Some(directory) = &screenshots {
                harness.render().unwrap().save(directory.join(format!("podcast-native-step-{step}.png"))).unwrap();
            }
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn workflow_file_selection_updates_the_correct_input() {
        let mut s = super::State::default();
        s.add_files(vec!["audio.wav".into()]);
        assert_eq!(s.master, "audio.wav");
        s.add_files(vec!["one.wav".into(), "two.wav".into()]);
        assert_eq!(s.master, "audio.wav");
        assert!(s.error.contains("one master"));
        s.step = 3;
        s.destination = 2;
        s.add_files(vec!["01.mov".into(), "02.mp4".into()]);
        assert_eq!(s.camera_b, ["01.mov", "02.mp4"]);
        assert!(s.camera_a.is_empty());
    }

    #[test]
    fn fresh_install_keeps_audio_and_returns_to_setup_before_creating_a_project() {
        let mut s = super::State { master: "my-recording.mp3".into(), ..Default::default() };
        s.setup.checked = true;
        s.import_audio(true);
        assert_eq!(s.step, 0);
        assert_eq!(s.master, "my-recording.mp3");
        assert!(s.project_id.is_empty());
        assert!(!s.busy());
        assert!(s.error.contains("Install"));
    }

    #[test]
    fn static_server_is_rejected_before_any_import_post() {
        let (endpoint, worker) = server_once("501 Not Implemented", "<!DOCTYPE HTML><title>Unsupported method ('POST')</title>");
        let state = super::State { endpoint, ..Default::default() };
        let client = super::Client::new(&state).unwrap();
        let error = client.post("/v1/projects", serde_json::json!({"name":"test"})).unwrap_err();
        assert_eq!(error, super::WRONG_SERVICE);
        assert!(worker.join().unwrap().starts_with("GET /health "));
        assert!(!error.contains("<"));
    }

    #[test]
    fn health_requires_helper_identity_and_protocol() {
        for (body, expected) in [
            (r#"{"status":"ok"}"#, false),
            (r#"{"status":"ok","service":"other","schema_version":1,"protocol_version":1}"#, false),
            (r#"{"status":"ok","service":"youcraft","schema_version":1,"protocol_version":1}"#, true),
            (r#"{"status":"ok","version":"0.1.0","schema_version":1}"#, false),
        ] {
            let (endpoint, worker) = server_once("200 OK", body);
            assert_eq!(super::helper_available(&endpoint), expected);
            worker.join().unwrap();
        }
        assert!(super::validate_endpoint("http://127.0.0.1:8000/folder/index.html").is_err());
        assert!(super::validate_endpoint("https://example.org").is_err());
    }

    #[test]
    fn error_pages_never_become_long_html_in_the_interface() {
        let status = reqwest::StatusCode::NOT_IMPLEMENTED;
        assert_eq!(super::response_value(status, "<!DOCTYPE HTML>server").unwrap_err(), super::WRONG_SERVICE);
        assert_eq!(super::response_value(reqwest::StatusCode::OK, "<html>files</html>").unwrap_err(), super::WRONG_SERVICE);
        assert!(super::display_error("No diarize provider configured; set BETUN_MODEL_CONFIG".into()).contains("Model setup"));
    }

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
    #[test]
    fn smile_calibration_uses_separated_sample_groups_not_a_fixed_cutoff() {
        let samples = serde_json::json!({"neutral":[{"value":0.02},{"value":0.04},{"value":0.06}],
            "smile":[{"value":0.18},{"value":0.22},{"value":0.24}]});
        assert!((super::calibrated_smile_threshold(&samples).unwrap() - 0.13).abs() < 1e-6);
        assert!(super::calibrated_smile_threshold(&serde_json::json!({})).is_err());
        let identical = serde_json::json!({"neutral":[{"value":0.2},{"value":0.2},{"value":0.2}],
            "smile":[{"value":0.21},{"value":0.21},{"value":0.21}]});
        assert!(super::calibrated_smile_threshold(&identical).is_err());
    }
}
