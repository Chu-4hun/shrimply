use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use reqwest::blocking::RequestBuilder;
use uuid::Uuid;

const JOB_HEADER: &str = "Shrimply-Job-ID";
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

struct State {
    server_url: String,
    job_id: Uuid,
    cancelled: AtomicBool,
    finished: AtomicBool,
    heartbeat_started: AtomicBool,
    heartbeat_thread: Mutex<Option<thread::Thread>>,
}

#[derive(Clone)]
pub struct CancellationToken {
    state: Arc<State>,
}

impl CancellationToken {
    pub fn new(server_url: &str) -> Result<Self, String> {
        let server_url = server_url.trim().trim_end_matches('/');
        if server_url.is_empty() {
            return Err("Server URL is empty".to_string());
        }
        Ok(Self {
            state: Arc::new(State {
                server_url: server_url.to_string(),
                job_id: Uuid::new_v4(),
                cancelled: AtomicBool::new(false),
                finished: AtomicBool::new(false),
                heartbeat_started: AtomicBool::new(false),
                heartbeat_thread: Mutex::new(None),
            }),
        })
    }

    pub fn job_id(&self) -> Uuid {
        self.state.job_id
    }

    pub fn is_cancelled(&self) -> bool {
        self.state.cancelled.load(Ordering::Acquire)
    }

    pub fn cancel(&self) {
        if self.state.cancelled.swap(true, Ordering::AcqRel)
            || self.state.finished.load(Ordering::Acquire)
        {
            return;
        }
        wake_heartbeat(&self.state);
        let state = self.state.clone();
        thread::Builder::new()
            .name(format!("compute-cancel:{}", state.job_id))
            .spawn(move || {
                let endpoint = format!("{}/compute/jobs/{}", state.server_url, state.job_id);
                match reqwest::blocking::Client::builder()
                    .connect_timeout(REQUEST_TIMEOUT)
                    .timeout(REQUEST_TIMEOUT)
                    .build()
                    .and_then(|client| client.delete(&endpoint).send())
                {
                    Ok(response) if response.status().is_success() => {
                        tracing::info!(job_id = %state.job_id, "Cancelled compute job");
                    }
                    Ok(response) => {
                        tracing::warn!(job_id = %state.job_id, status = %response.status(), "Compute cancellation was rejected");
                    }
                    Err(error) => {
                        tracing::warn!(job_id = %state.job_id, %error, "Could not send compute cancellation");
                    }
                }
            })
            .expect("failed to start compute cancellation thread");
    }

    pub fn manage(
        &self,
        request: RequestBuilder,
    ) -> Result<(RequestBuilder, ManagedJobGuard), String> {
        if self.is_cancelled() {
            return Err("Compute job cancelled".to_string());
        }
        if self.state.heartbeat_started.swap(true, Ordering::AcqRel) {
            return Err("Compute cancellation token was already used".to_string());
        }
        let state = self.state.clone();
        let thread = thread::Builder::new()
            .name(format!("compute-heartbeat:{}", state.job_id))
            .spawn(move || heartbeat_loop(state))
            .map_err(|error| format!("Could not start compute heartbeat: {error}"))?;
        Ok((
            request.header(JOB_HEADER, self.state.job_id.to_string()),
            ManagedJobGuard {
                state: self.state.clone(),
                thread: Some(thread),
            },
        ))
    }
}

pub struct ManagedJobGuard {
    state: Arc<State>,
    thread: Option<JoinHandle<()>>,
}

impl ManagedJobGuard {
    fn stop(&mut self) {
        self.state.finished.store(true, Ordering::Release);
        wake_heartbeat(&self.state);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for ManagedJobGuard {
    fn drop(&mut self) {
        self.stop();
    }
}

fn wake_heartbeat(state: &State) {
    if let Some(thread) = state
        .heartbeat_thread
        .lock()
        .expect("compute heartbeat lock poisoned")
        .as_ref()
    {
        thread.unpark();
    }
}

fn heartbeat_loop(state: Arc<State>) {
    *state
        .heartbeat_thread
        .lock()
        .expect("compute heartbeat lock poisoned") = Some(thread::current());
    while !state.finished.load(Ordering::Acquire) && !state.cancelled.load(Ordering::Acquire) {
        thread::park_timeout(HEARTBEAT_INTERVAL);
        if state.finished.load(Ordering::Acquire) || state.cancelled.load(Ordering::Acquire) {
            break;
        }
        let endpoint = format!(
            "{}/compute/jobs/{}/heartbeat",
            state.server_url, state.job_id
        );
        match reqwest::blocking::Client::builder()
            .connect_timeout(REQUEST_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .and_then(|client| client.put(&endpoint).send())
        {
            Ok(response) if response.status().is_success() => {}
            Ok(response) => tracing::warn!(
                job_id = %state.job_id,
                status = %response.status(),
                "Compute heartbeat was rejected"
            ),
            Err(error) => tracing::warn!(
                job_id = %state.job_id,
                %error,
                "Compute heartbeat failed"
            ),
        }
    }
    *state
        .heartbeat_thread
        .lock()
        .expect("compute heartbeat lock poisoned") = None;
}

pub fn queued_status(position: usize) -> String {
    if position <= 1 {
        "Queued".to_string()
    } else {
        format!("Queued · {} ahead", position - 1)
    }
}
