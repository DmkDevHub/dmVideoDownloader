use dm_video_protocol::{pipe, Ack, Body, Candidate, Code, Message, SourceType};
use serde::Serialize;
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

pub struct Bridge {
    inbox: Mutex<Vec<(String, Candidate)>>,
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
    pub fn new() -> Self {
        Self {
            inbox: Mutex::new(Vec::new()),
        }
    }

    pub fn start(
        self: &Arc<Self>,
        root: &Path,
        authorize: impl Fn(&str) -> Result<(), Code> + Send + 'static,
    ) -> std::io::Result<()> {
        let server = pipe::Server::bind(root)?;
        let bridge = Arc::clone(self);
        std::thread::Builder::new()
            .name("native-bridge".into())
            .spawn(move || {
                server.run(
                    move |_, id, message| match authorize(id) {
                        Ok(()) => bridge.receive(message),
                        Err(code) => Message::error(message.request_id, code),
                    },
                    |_| {},
                );
            })?;
        Ok(())
    }
    fn receive(&self, message: Message) -> Message {
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
            _ => Message::error(id, Code::ConnectionTestNotWaiting),
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
