#![cfg_attr(windows, windows_subsystem = "windows")]
use dm_video_protocol::{extension_origin, frame, pipe, Code, Message};
use std::time::Duration;

fn main() {
    let origin = std::env::args()
        .nth(1)
        .and_then(|argument| extension_origin(&argument).map(str::to_owned));
    let root = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(std::path::Path::to_path_buf));
    let mut client: Option<pipe::Client> = None;
    let result = frame::exchange(
        &mut std::io::stdin().lock(),
        &mut std::io::stdout().lock(),
        |request| {
            let reply = (|| {
                frame::request_only(&request)?;
                let origin = origin.as_deref().ok_or(Code::ExtensionIdMismatch)?;
                let root = root.as_deref().ok_or(Code::HostPathMismatch)?;
                dm_video_protocol::manifest::Manifest::verify(root, origin)?;
                if client.is_none() {
                    client = Some(pipe::Client::connect(
                        root,
                        origin,
                        Duration::from_millis(500),
                    )?);
                }
                client.as_mut().unwrap().exchange(&request)
            })();
            reply.unwrap_or_else(|code| {
                client = None;
                Message::error(request.request_id, code)
            })
        },
    );
    if result.is_err() {
        eprintln!("NATIVE_FRAME_FAILED");
        std::process::exit(1);
    }
}
