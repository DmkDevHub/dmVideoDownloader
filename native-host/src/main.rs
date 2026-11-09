use dm_video_protocol::{frame, Code, Message};

fn main() {
    let result = frame::exchange(
        &mut std::io::stdin().lock(),
        &mut std::io::stdout().lock(),
        |request| {
            let code = frame::request_only(&request)
                .err()
                .unwrap_or(Code::AppNotRunning);
            Message::error(request.request_id, code)
        },
    );
    if result.is_err() {
        eprintln!("NATIVE_FRAME_FAILED");
        std::process::exit(1);
    }
}
