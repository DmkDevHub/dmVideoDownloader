use crate::connection::{Connection, Status};
use dm_video_protocol::{pipe, Ack, Body, Candidate, Code, Message, RuntimeStatus, SourceType};
use serde::Serialize;
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};

pub struct Bridge {
    inbox: Mutex<Vec<(String, Candidate)>>,
    connection: Mutex<Connection>,
    listening: AtomicBool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateRow {
    id: String,
    title: String,
    source_type: SourceType,
}

#[tauri::command]
pub fn received_candidates(bridge: tauri::State<Arc<Bridge>>) -> Result<Vec<CandidateRow>, String> {
    let inbox = bridge.inbox.lock().map_err(|_| "BRIDGE_BUSY")?;
    Ok(inbox
        .iter()
        .rev()
        .map(|(id, candidate)| CandidateRow {
            id: id.clone(),
            title: candidate
                .page_title
                .as_ref()
                .filter(|title| !title.trim().is_empty() && !title.contains("://"))
                .cloned()
                .unwrap_or_else(|| "Chrome 영상 후보".into()),
            source_type: candidate.source_type.clone(),
        })
        .collect())
}
impl Bridge {
    pub fn candidate(&self, id: &str) -> Result<Candidate, &'static str> {
        let inbox = self.inbox.lock().map_err(|_| "BRIDGE_BUSY")?;
        inbox
            .iter()
            .find(|(request_id, _)| request_id == id)
            .map(|(_, candidate)| candidate.clone())
            .ok_or("CANDIDATE_MISSING")
    }
    pub fn new() -> Self {
        Self {
            inbox: Mutex::new(Vec::new()),
            connection: Mutex::new(Connection::new()),
            listening: AtomicBool::new(false),
        }
    }

    pub fn start(
        self: &Arc<Self>,
        root: &Path,
        authorize: impl Fn(&str) -> Result<bool, Code> + Send + 'static,
    ) -> std::io::Result<()> {
        let server = pipe::Server::bind(root)?;
        let bridge = Arc::clone(self);
        self.listening.store(true, Ordering::Release);
        let result = std::thread::Builder::new()
            .name("native-bridge".into())
            .spawn(move || {
                let receiver = Arc::clone(&bridge);
                let closer = Arc::clone(&bridge);
                server.run(
                    move |connection, id, message| match authorize(id) {
                        Ok(auto_detect) => receiver.receive(connection, id, auto_detect, message),
                        Err(code) => Message::error(message.request_id, code),
                    },
                    move |connection| {
                        if let Ok(mut state) = closer.connection.lock() {
                            state.closed(connection);
                        }
                    },
                );
                bridge.listening.store(false, Ordering::Release);
                bridge.invalidate();
            });
        if result.is_err() {
            self.listening.store(false, Ordering::Release);
        }
        result.map(|_| ())
    }
    pub fn invalidate(&self) {
        if let Ok(mut state) = self.connection.lock() {
            state.reset();
        }
    }
    pub fn registration_lost(&self, code: Code) {
        if let Ok(mut state) = self.connection.lock() {
            state.registration_lost(code);
        }
    }
    pub fn begin_test(&self) -> Result<Status, &'static str> {
        if !self.listening.load(Ordering::Acquire) {
            return Err("HOST_START_FAILED");
        }
        self.connection
            .lock()
            .map_err(|_| "BRIDGE_BUSY")?
            .begin(Instant::now())
    }
    pub fn status(&self) -> Result<Status, &'static str> {
        Ok(self
            .connection
            .lock()
            .map_err(|_| "BRIDGE_BUSY")?
            .status(Instant::now()))
    }
    fn receive(
        &self,
        connection: u64,
        extension_id: &str,
        auto_detect: bool,
        message: Message,
    ) -> Message {
        if matches!(message.body, Body::Ping(_) | Body::ConnectionTestResult(_)) {
            return match self.connection.lock() {
                Ok(mut state) => state.handle(Instant::now(), connection, extension_id, message),
                Err(_) => Message::error(message.request_id, Code::HostStartFailed),
            };
        }
        let id = message.request_id;
        match message.body {
            Body::VideoCandidate(candidate) => {
                let Ok(mut inbox) = self.inbox.lock() else {
                    return Message::error(id, Code::HostStartFailed);
                };
                if let Some((_, previous)) = inbox.iter().find(|(request_id, _)| request_id == &id)
                {
                    if previous.candidate_id != candidate.candidate_id {
                        return Message::error(id, Code::InvalidPayload);
                    }
                } else {
                    if inbox.len() >= 50 {
                        return Message::error(id, Code::InvalidPayload);
                    }
                    inbox.push((id.clone(), candidate.clone()));
                }
                Message::new(
                    id,
                    Body::Ack(Ack {
                        accepted: true,
                        candidate_id: Some(candidate.candidate_id),
                    }),
                )
            }
            Body::GetRuntimeStatus(_) => Message::new(
                id,
                Body::RuntimeStatus(RuntimeStatus {
                    protocol_version: 1,
                    app_reachable: true,
                    browser_auto_detect: auto_detect,
                }),
            ),
            _ => Message::error(id, Code::InvalidPayload),
        }
    }
}

#[cfg(feature = "smoke-test")]
#[tauri::command]
pub fn smoke_bridge_received(bridge: tauri::State<Arc<Bridge>>) -> bool {
    let Some(expected) = std::env::var_os("DM_SMOKE_ROOT").and_then(|root| {
        std::fs::read_to_string(std::path::PathBuf::from(root).join("bridge-ack")).ok()
    }) else {
        return false;
    };
    bridge
        .inbox
        .lock()
        .is_ok_and(|inbox| inbox.len() == 1 && inbox[0].1.candidate_id == expected)
}
