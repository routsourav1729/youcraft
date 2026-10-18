//! First-run actions stay visible; installers run outside the UI thread.
use serde_json::Value;
use std::{
    io::Read,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
};
#[cfg(any(windows, test))]
use std::{
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};

#[derive(Default)]
pub struct State {
    pub check_on_start: bool,
    pub checked: bool,
    pub advance_on_ready: bool,
    pub capabilities: Value,
    pub ai_open: bool,
    pub hf_token: String,
    pub stage: String,
    pub error: String,
    pub progress: Value,
    task: Option<Task>,
}
struct Task {
    result: Receiver<Result<(), String>>,
    stop: Arc<AtomicBool>,
    status: PathBuf,
}
impl Drop for Task {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
impl State {
    pub fn busy(&self) -> bool {
        self.task.is_some()
    }
    pub fn cancel(&mut self) {
        if let Some(task) = &self.task {
            task.stop.store(true, Ordering::Relaxed);
        }
    }
    pub fn ready(&self) -> bool {
        self.capabilities["ready"] == true
    }
    pub fn start(&mut self, script: &str, option: &str, value: &str) {
        if self.busy() {
            return;
        }
        self.advance_on_ready = script == "Setup-models.ps1";
        match launch(script, option, value, &self.hf_token) {
            Ok(task) => {
                self.task = Some(task);
                self.stage = "Starting…".into();
                self.error.clear();
                self.progress = Value::Null;
            }
            Err(error) => self.error = error,
        }
        self.hf_token.clear();
    }
    pub fn poll(&mut self) -> bool {
        let Some(task) = &self.task else { return false };
        if let Ok(file) = std::fs::File::open(&task.status) {
            let mut text = String::new();
            if file.take(8192).read_to_string(&mut text).is_ok()
                && let Ok(value) = serde_json::from_str::<Value>(&text)
            {
                self.stage = value["stage"].as_str().unwrap_or("Installing…").into();
                self.error = value["error"].as_str().unwrap_or("").into();
                self.progress = value;
            }
        }
        match task.result.try_recv() {
            Ok(result) => {
                self.task = None;
                if let Err(error) = result
                    && self.error.is_empty()
                {
                    self.error = error;
                }
                true
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.task = None;
                self.error = "Setup stopped unexpectedly. Retry setup.".into();
                true
            }
            Err(mpsc::TryRecvError::Empty) => false,
        }
    }
}

#[cfg(windows)]
fn launch(script: &str, option: &str, value: &str, token: &str) -> Result<Task, String> {
    use std::os::windows::process::CommandExt;
    let directory = std::env::current_exe().map_err(|e| e.to_string())?.parent().map(Path::to_path_buf).ok_or("Installation folder missing")?;
    let script = directory.join(script);
    if !script.is_file() {
        return Err("Setup helper missing. Reinstall YouCraft.".into());
    }
    let root = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).ok_or("Windows app data folder missing")?.join("YouCraft");
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let is_model = script.file_name().is_some_and(|name| name == "Setup-models.ps1");
    let status = root.join(if is_model { "setup-status.json" } else { "ai-status.json" });
    let log = root.join(if is_model { "setup.log" } else { "ai-setup.log" });
    let _ = std::fs::remove_file(&status);
    let mut command = Command::new("powershell.exe");
    command
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(script)
        .args([option, value, "-StatusFile"])
        .arg(&status)
        .creation_flags(0x0800_0000);
    if !token.trim().is_empty() {
        command.env("HF_TOKEN", token.trim());
    }
    spawn(command, status, &log)
}
#[cfg(not(windows))]
fn launch(_script: &str, _option: &str, _value: &str, _token: &str) -> Result<Task, String> {
    Err("Model and AI setup runs in the installed Windows app.".into())
}

#[cfg(any(windows, test))]
fn spawn(mut command: Command, status: PathBuf, log: &Path) -> Result<Task, String> {
    let output = std::fs::File::create(log).map_err(|e| e.to_string())?;
    command.stdin(Stdio::null()).stdout(output.try_clone().map_err(|e| e.to_string())?).stderr(output);
    let mut child = command.spawn().map_err(|e| format!("Cannot start setup: {e}"))?;
    let (tx, rx) = mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let cancelled = Arc::clone(&stop);
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<(), String> {
            loop {
                if cancelled.load(Ordering::Relaxed) {
                    #[cfg(windows)]
                    {
                        use std::os::windows::process::CommandExt;
                        let _ = Command::new("taskkill")
                            .args(["/PID", &child.id().to_string(), "/T", "/F"])
                            .creation_flags(0x0800_0000)
                            .stdout(Stdio::null())
                            .stderr(Stdio::null())
                            .status();
                    }
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("Setup cancelled. Downloaded files are kept for retry.".into());
                }
                match child.try_wait().map_err(|e| e.to_string())? {
                    Some(status) if status.success() => return Ok(()),
                    Some(_) => return Err("Setup failed. Open setup log for details.".into()),
                    None => std::thread::sleep(Duration::from_millis(100)),
                }
            }
        }))
        .unwrap_or_else(|_| Err("Setup worker failed".into()));
        let _ = tx.send(result);
    });
    Ok(Task { result: rx, stop, status })
}

pub fn log_path() -> PathBuf {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_default().join("YouCraft/setup.log")
}

#[cfg(test)]
mod tests {
    #[cfg(not(windows))]
    #[test]
    fn failed_installer_exits_and_reports_without_blocking_ui() {
        let root = std::env::temp_dir().join(format!("youcraft-setup-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let mut command = super::Command::new("sh");
        command.args(["-c", "echo dependency-failed; exit 7"]);
        let task = super::spawn(command, root.join("status.json"), &root.join("setup.log")).unwrap();
        let mut state = super::State { task: Some(task), ..Default::default() };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !state.poll() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(!state.busy());
        assert!(state.error.contains("Setup failed"));
        assert!(std::fs::read_to_string(root.join("setup.log")).unwrap().contains("dependency-failed"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
