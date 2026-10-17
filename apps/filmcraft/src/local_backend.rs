//! Start the packaged analysis helper on this computer. Developer builds may supply their own.
#[cfg(windows)]
pub struct LocalBackend(std::process::Child);

#[cfg(windows)]
pub fn start() -> Option<LocalBackend> {
    use std::{
        net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream},
        os::windows::process::CommandExt,
        process::{Command, Stdio},
        time::Duration,
    };
    if std::env::var_os("AMA_DISABLE_LOCAL_BACKEND").is_some() {
        return None;
    }
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8765);
    if TcpStream::connect_timeout(&address, Duration::from_millis(150)).is_ok() {
        return None; // Never take ownership of an existing helper.
    }
    let executable = std::env::current_exe().ok()?;
    let directory = executable.parent()?;
    let backend = directory.join("BetunBackend/BetunBackend.exe");
    if !backend.is_file() {
        return None;
    }
    let mut command = Command::new(backend);
    command
        .current_dir(directory)
        .env("BETUN_NO_BROWSER", "1")
        .env("BETUN_FILMCRAFT_CLI", directory.join("filmcraft-cli.exe"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(0x0800_0000);
    if let Some(root) = std::env::var_os("LOCALAPPDATA") {
        let root = std::path::PathBuf::from(root);
        let current = root.join("YouCraft/models.json");
        let config = if current.is_file() { current } else { root.join("AmaBartalap/models.json") };
        if config.is_file() && std::env::var_os("BETUN_MODEL_CONFIG").is_none() {
            command.env("BETUN_MODEL_CONFIG", config);
        }
    }
    match command.spawn() {
        Ok(child) => Some(LocalBackend(child)),
        Err(error) => {
            log::error!("Could not start local analysis: {error}");
            None
        }
    }
}

#[cfg(windows)]
impl Drop for LocalBackend {
    fn drop(&mut self) {
        use std::os::windows::process::CommandExt;
        if matches!(self.0.try_wait(), Ok(None)) {
            // Include disposable model workers, so closing the editor releases their GPU memory.
            let stopped = std::process::Command::new("taskkill")
                .args(["/PID", &self.0.id().to_string(), "/T", "/F"])
                .creation_flags(0x0800_0000)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            if !matches!(stopped, Ok(status) if status.success()) {
                log::warn!("Could not stop local analysis tree: {stopped:?}");
                if let Err(error) = self.0.kill() {
                    log::warn!("Could not stop local analysis: {error}");
                    return;
                }
            }
            if let Err(error) = self.0.wait() {
                log::warn!("Could not reap local analysis: {error}");
            }
        }
    }
}

#[cfg(not(windows))]
pub fn start() -> Option<()> {
    None
}
