use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::protocol::{BridgeCommand, BridgeRequest, BridgeResponse};

const SOCKET_WRITE_TIMEOUT: Duration = Duration::from_secs(5);
const SOCKET_RESPONSE_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const SOCKET_CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug)]
pub enum BridgeError {
    Transport(String),
    Rejected(String),
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(error) | Self::Rejected(error) => formatter.write_str(error),
        }
    }
}

impl std::error::Error for BridgeError {}

pub fn socket_path(pid: u32) -> Result<PathBuf, String> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| "XDG_RUNTIME_DIR is not set".to_string())?;
    Ok(runtime.join("shrimply").join(format!("mcp-{pid}.sock")))
}

#[derive(Clone)]
pub struct Bridge {
    project_path: PathBuf,
    socket_path: PathBuf,
}

impl Bridge {
    pub fn connect(project_path: &Path) -> Result<Self, BridgeError> {
        let project_path = shrimply_project::project::normalized_project_path(project_path);
        if project_path.to_str().is_none() {
            return Err(BridgeError::Transport(
                "project path is not valid UTF-8".to_string(),
            ));
        }
        let pid = shrimply_project::project::project_lock_owner(&project_path)
            .map_err(BridgeError::Transport)?
            .ok_or_else(|| {
                BridgeError::Transport(format!(
                    "{} is not open in Shrimply",
                    project_path.display()
                ))
            })?;
        let bridge = Self {
            project_path,
            socket_path: socket_path(pid).map_err(BridgeError::Transport)?,
        };
        bridge.request(BridgeCommand::Handshake)?;
        Ok(bridge)
    }

    pub fn request(&self, command: BridgeCommand) -> Result<serde_json::Value, BridgeError> {
        self.request_with_cancel(command, Arc::new(AtomicBool::new(false)))
    }

    pub fn request_with_cancel(
        &self,
        command: BridgeCommand,
        canceled: Arc<AtomicBool>,
    ) -> Result<serde_json::Value, BridgeError> {
        let mut stream = UnixStream::connect(&self.socket_path).map_err(|error| {
            BridgeError::Transport(format!(
                "could not connect to the open editor at {}: {error}",
                self.socket_path.display()
            ))
        })?;
        stream
            .set_write_timeout(Some(SOCKET_CANCEL_POLL_INTERVAL))
            .map_err(|error| {
                BridgeError::Transport(format!("could not configure editor socket: {error}"))
            })?;
        stream
            .set_read_timeout(Some(SOCKET_CANCEL_POLL_INTERVAL))
            .map_err(|error| {
                BridgeError::Transport(format!("could not configure editor socket: {error}"))
            })?;
        let request = BridgeRequest {
            project_path: self
                .project_path
                .to_str()
                .expect("project path was validated when the bridge connected")
                .to_string(),
            command,
        };
        let mut encoded = serde_json::to_vec(&request).map_err(|error| {
            BridgeError::Transport(format!("could not encode editor request: {error}"))
        })?;
        encoded.push(b'\n');
        let write_deadline = Instant::now() + SOCKET_WRITE_TIMEOUT;
        let mut written = 0;
        while written < encoded.len() {
            check_canceled(&canceled)?;
            match stream.write(&encoded[written..]) {
                Ok(0) => {
                    return Err(BridgeError::Transport(
                        "the editor closed the MCP bridge while receiving a request".to_string(),
                    ));
                }
                Ok(count) => written += count,
                Err(error) if is_timeout(&error) && Instant::now() < write_deadline => {}
                Err(error) => {
                    return Err(BridgeError::Transport(format!(
                        "could not send editor request: {error}"
                    )));
                }
            }
        }
        let mut line = String::new();
        let mut reader = BufReader::new(stream);
        let response_deadline = Instant::now() + SOCKET_RESPONSE_TIMEOUT;
        loop {
            check_canceled(&canceled)?;
            match reader.read_line(&mut line) {
                Ok(_) => break,
                Err(error) if is_timeout(&error) && Instant::now() < response_deadline => {}
                Err(error) => {
                    return Err(BridgeError::Transport(format!(
                        "could not read editor response: {error}"
                    )));
                }
            }
        }
        if line.is_empty() {
            return Err(BridgeError::Transport(
                "the editor closed the MCP bridge without a response".to_string(),
            ));
        }
        let response: BridgeResponse = serde_json::from_str(&line).map_err(|error| {
            BridgeError::Transport(format!("editor returned malformed bridge JSON: {error}"))
        })?;
        if response.project_path.is_empty()
            && let Some(error) = &response.error
        {
            return Err(BridgeError::Transport(error.clone()));
        }
        if response.project_path
            != self
                .project_path
                .to_str()
                .expect("project path was validated when the bridge connected")
        {
            return Err(BridgeError::Rejected(format!(
                "editor handshake named {}, expected {}",
                response.project_path,
                self.project_path.display()
            )));
        }
        match (response.result, response.error) {
            (Some(result), None) => Ok(result),
            (None, Some(error)) => Err(BridgeError::Rejected(error)),
            _ => Err(BridgeError::Transport(
                "editor returned an invalid bridge response".to_string(),
            )),
        }
    }
}

fn check_canceled(canceled: &AtomicBool) -> Result<(), BridgeError> {
    if canceled.load(Ordering::Acquire) {
        Err(BridgeError::Rejected(
            "MCP request was canceled".to_string(),
        ))
    } else {
        Ok(())
    }
}

fn is_timeout(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
    )
}
