//! Own only the packaged helper we start; never confuse an unrelated local server with it.
#[cfg(windows)]
use std::path::PathBuf;

#[derive(Clone, Default)]
pub struct Connection {
    pub endpoint: String,
    pub token: String,
    pub error: String,
}

pub struct LocalBackend {
    pub connection: Connection,
    #[cfg(windows)]
    child: Option<std::process::Child>,
}

#[cfg(any(windows, test))]
fn available_port(preferred: u16) -> Result<u16, String> {
    use std::net::{Ipv4Addr, TcpListener};
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, preferred))
        .or_else(|_| TcpListener::bind((Ipv4Addr::LOCALHOST, 0)))
        .map_err(|e| format!("Cannot reserve a local analysis port: {e}"))?;
    listener.local_addr().map(|a| a.port()).map_err(|e| e.to_string())
}

#[cfg(windows)]
pub fn start() -> Option<LocalBackend> {
    if std::env::var_os("AMA_DISABLE_LOCAL_BACKEND").is_some() {
        return None;
    }
    let directory = std::env::current_exe().ok().and_then(|p| p.parent().map(std::path::Path::to_path_buf));
    Some(start_at(directory.as_deref()))
}

#[cfg(windows)]
fn start_at(directory: Option<&std::path::Path>) -> LocalBackend {
    use std::{
        os::windows::process::CommandExt,
        process::{Command, Stdio},
    };
    let mut backend = LocalBackend { connection: Connection::default(), child: None };
    backend.connection.endpoint = "http://127.0.0.1:8765".into();
    backend.connection.token = filmcraft_ui_egui::podcast::local_token();
    if filmcraft_ui_egui::podcast::helper_available(&backend.connection.endpoint) {
        return backend;
    }
    let result = (|| -> Result<std::process::Child, String> {
        let directory = directory.ok_or("Cannot locate YouCraft installation")?;
        let helper = directory.join("BetunBackend/BetunBackend.exe");
        if !helper.is_file() {
            return Err("Local helper missing. Reinstall YouCraft.".into());
        }
        let port = available_port(8765)?;
        backend.connection.endpoint = format!("http://127.0.0.1:{port}");
        let mut command = Command::new(helper);
        command
            .current_dir(directory)
            .args(["--port", &port.to_string()])
            .env("BETUN_NO_BROWSER", "1")
            .env("BETUN_FILMCRAFT_CLI", directory.join("filmcraft-cli.exe"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .creation_flags(0x0800_0000);
        // Keep startup diagnostics outside the installation, instead of discarding them.
        if let Some(root) = std::env::var_os("LOCALAPPDATA") {
            let root = PathBuf::from(root);
            let logs = root.join("YouCraft/logs");
            if std::fs::create_dir_all(&logs).is_ok()
                && let Ok(log) = std::fs::File::create(logs.join("analysis-helper.log"))
            {
                command.stderr(Stdio::from(log));
            } else {
                command.stderr(Stdio::null());
            }
        }
        command.spawn().map_err(|e| format!("Cannot start local analysis: {e}"))
    })();
    match result {
        Ok(child) => backend.child = Some(child),
        Err(error) => {
            log::error!("{error}");
            backend.connection.error = error;
        }
    }
    backend
}

#[cfg(windows)]
impl Drop for LocalBackend {
    fn drop(&mut self) {
        use std::os::windows::process::CommandExt;
        let Some(child) = self.child.as_mut() else { return };
        if matches!(child.try_wait(), Ok(None)) {
            let stopped = std::process::Command::new("taskkill")
                .args(["/PID", &child.id().to_string(), "/T", "/F"])
                .creation_flags(0x0800_0000)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            if !matches!(stopped, Ok(status) if status.success()) {
                log::warn!("Could not stop local analysis tree: {stopped:?}");
                if let Err(error) = child.kill() {
                    log::warn!("Could not stop local analysis: {error}");
                    return;
                }
            }
            if let Err(error) = child.wait() {
                log::warn!("Could not reap local analysis: {error}");
            }
        }
    }
}

#[cfg(not(windows))]
pub fn start() -> Option<LocalBackend> {
    None
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    #[test]
    fn packaged_helper_starts_beside_an_unrelated_static_server() {
        use std::{
            io::{Read, Write},
            sync::{
                Arc,
                atomic::{AtomicBool, Ordering},
            },
            time::{Duration, Instant},
        };
        let Ok(directory) = std::env::var("YOUCRAFT_INSTALLED_SMOKE") else { return };
        let unrelated = std::net::TcpListener::bind("127.0.0.1:8765").unwrap();
        unrelated.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let done = Arc::clone(&stop);
        let server = std::thread::spawn(move || {
            while !done.load(Ordering::Relaxed) {
                if let Ok((mut socket, _)) = unrelated.accept() {
                    socket.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
                    let mut buffer = [0; 4096];
                    let _ = socket.read(&mut buffer);
                    let _ = socket.write_all(b"HTTP/1.1 501 Not Implemented\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        });
        let backend = super::start_at(Some(std::path::Path::new(&directory)));
        assert!(backend.connection.error.is_empty(), "{}", backend.connection.error);
        assert_ne!(backend.connection.endpoint, "http://127.0.0.1:8765");
        let deadline = Instant::now() + Duration::from_secs(60);
        while !filmcraft_ui_egui::podcast::helper_available(&backend.connection.endpoint) {
            assert!(Instant::now() < deadline, "packaged helper did not start");
            std::thread::sleep(Duration::from_millis(100));
        }
        drop(backend);
        assert!(std::net::TcpStream::connect("127.0.0.1:8765").is_ok());
        stop.store(true, Ordering::Relaxed);
        server.join().unwrap();
    }

    #[test]
    fn occupied_port_is_not_reused_or_terminated() {
        let unrelated = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let occupied = unrelated.local_addr().unwrap().port();
        let chosen = super::available_port(occupied).unwrap();
        assert_ne!(chosen, occupied);
        assert!(std::net::TcpStream::connect(unrelated.local_addr().unwrap()).is_ok());
    }
}
