use dm_video_protocol::{pipe, Ack, Body, Candidate, Code, Message};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

pub struct Bridge {
    inbox: Mutex<Vec<(String, Candidate)>>,
}
impl Bridge {
    pub fn new() -> Self {
        Self {
            inbox: Mutex::new(Vec::new()),
        }
    }

    pub fn start(self: &Arc<Self>, root: &Path) -> std::io::Result<()> {
        let server = pipe::Server::bind(root)?;
        let bridge = Arc::clone(self);
        std::thread::Builder::new()
            .name("native-bridge".into())
            .spawn(move || {
                server.run(move |_, _, message| bridge.receive(message), |_| {});
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
    if !std::env::var_os("DM_SMOKE_ROOT")
        .is_some_and(|root| std::path::PathBuf::from(root).join("bridge-ack").is_file())
    {
        return false;
    }
    bridge
        .inbox
        .lock()
        .is_ok_and(|inbox| inbox.len() == 1 && inbox[0].1.candidate_id == "bridge-row")
}
