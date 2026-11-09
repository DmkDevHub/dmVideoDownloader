use dm_video_protocol::{utc_now, Ack, Body, Code, Message, Pong};
use serde::Serialize;
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

struct Challenge {
    connection: u64,
    request_id: String,
}
enum State {
    NeedsCheck,
    Waiting {
        until: Instant,
        challenge: Option<Challenge>,
    },
    Confirmed(u64),
    Disconnected,
    Failed(Code),
}
pub struct Connection {
    state: State,
    last_success: Option<(String, String)>,
    seen: VecDeque<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    phase: &'static str,
    round_trip_confirmed: bool,
    pub last_success_at: Option<String>,
    last_verified_id: Option<String>,
    remaining_seconds: Option<u64>,
    error_code: Option<Code>,
}

impl Connection {
    pub fn new() -> Self {
        Self {
            state: State::NeedsCheck,
            last_success: None,
            seen: VecDeque::new(),
        }
    }
    fn expire(&mut self, now: Instant) {
        if matches!(self.state, State::Waiting { until, .. } if until <= now) {
            self.state = State::Failed(Code::ConnectionTestTimeout);
        }
    }
    pub fn begin(&mut self, now: Instant) -> Result<Status, &'static str> {
        self.expire(now);
        if matches!(self.state, State::Waiting { .. }) {
            return Err("CONNECTION_TEST_BUSY");
        }
        self.state = State::Waiting {
            until: now + Duration::from_secs(30),
            challenge: None,
        };
        Ok(self.status(now))
    }
    pub fn reset(&mut self) {
        self.state = State::NeedsCheck;
    }
    pub fn status(&mut self, now: Instant) -> Status {
        self.expire(now);
        let (phase, remaining_seconds, error_code) = match self.state {
            State::NeedsCheck => ("needsCheck", None, None),
            State::Waiting { until, .. } => (
                "waiting",
                Some(until.saturating_duration_since(now).as_secs_f64().ceil() as u64),
                None,
            ),
            State::Confirmed(_) => ("connected", None, None),
            State::Disconnected => ("disconnected", None, None),
            State::Failed(code) => ("failed", None, Some(code)),
        };
        Status {
            phase,
            round_trip_confirmed: matches!(self.state, State::Confirmed(_)),
            last_success_at: self.last_success.as_ref().map(|(time, _)| time.clone()),
            last_verified_id: self.last_success.as_ref().map(|(_, id)| id.clone()),
            remaining_seconds,
            error_code,
        }
    }
    pub fn closed(&mut self, connection: u64) {
        if matches!(self.state, State::Confirmed(owner) if owner == connection) {
            self.state = State::Disconnected;
        } else if matches!(&self.state, State::Waiting { challenge: Some(owner), .. } if owner.connection == connection)
        {
            self.state = State::Failed(Code::HostStartFailed);
        }
    }
    pub fn handle(
        &mut self,
        now: Instant,
        connection: u64,
        extension_id: &str,
        message: Message,
    ) -> Message {
        self.expire(now);
        let id = message.request_id;
        let State::Waiting { challenge, .. } = &mut self.state else {
            let code = if matches!(self.state, State::Failed(Code::ConnectionTestTimeout)) {
                Code::ConnectionTestTimeout
            } else {
                Code::ConnectionTestNotWaiting
            };
            return Message::error(id, code);
        };
        match message.body {
            Body::Ping(_) if challenge.is_none() && !self.seen.contains(&id) => {
                if self.seen.len() == 128 {
                    self.seen.pop_front();
                }
                self.seen.push_back(id.clone());
                *challenge = Some(Challenge {
                    connection,
                    request_id: id.clone(),
                });
                Message::new(
                    id,
                    Body::Pong(Pong {
                        protocol_version: 1,
                        app_reachable: true,
                    }),
                )
            }
            Body::ConnectionTestResult(result)
                if result.success
                    && challenge.as_ref().is_some_and(|owner| {
                        owner.connection == connection && owner.request_id == id
                    }) =>
            {
                self.state = State::Confirmed(connection);
                self.last_success = Some((utc_now(), extension_id.into()));
                Message::new(
                    id,
                    Body::Ack(Ack {
                        accepted: true,
                        candidate_id: None,
                    }),
                )
            }
            _ => Message::error(id, Code::InvalidPayload),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dm_video_protocol::{Ping, TestResult};
    fn ping() -> Message {
        Message::new(
            "one",
            Body::Ping(Ping {
                purpose: "connection_test".into(),
            }),
        )
    }
    fn finish() -> Message {
        Message::new(
            "one",
            Body::ConnectionTestResult(TestResult { success: true }),
        )
    }
    #[test]
    fn confirms_only_matching_round_trip_and_keeps_history_after_disconnect() {
        let mut state = Connection::new();
        let now = Instant::now();
        assert!(matches!(
            state.handle(now, 1, "id", ping()).body,
            Body::Error(_)
        ));
        state.begin(now).unwrap();
        assert!(state.begin(now).is_err());
        assert!(matches!(
            state.handle(now, 1, "id", finish()).body,
            Body::Error(_)
        ));
        assert!(matches!(
            state.handle(now, 1, "id", ping()).body,
            Body::Pong(_)
        ));
        assert!(!state.status(now).round_trip_confirmed);
        assert!(matches!(
            state.handle(now, 2, "id", finish()).body,
            Body::Error(_)
        ));
        assert!(matches!(
            state.handle(now, 1, "id", finish()).body,
            Body::Ack(_)
        ));
        assert!(state.status(now).round_trip_confirmed);
        state.closed(1);
        let status = state.status(now);
        assert_eq!(status.phase, "disconnected");
        assert!(status.last_success_at.is_some());
        state.begin(now).unwrap();
        assert!(matches!(
            state.handle(now, 3, "id", ping()).body,
            Body::Error(_)
        ));
    }
    #[test]
    fn exact_deadline_and_disconnect_during_wait_reject_late_ack() {
        let mut state = Connection::new();
        let now = Instant::now();
        state.begin(now).unwrap();
        state.handle(now, 1, "id", ping());
        let late = now + Duration::from_secs(30);
        assert!(
            matches!(state.handle(late, 1, "id", finish()).body, Body::Error(err) if err.code == Code::ConnectionTestTimeout)
        );
        assert!(state.status(late).last_success_at.is_none());
        state.begin(late).unwrap();
        state.handle(
            late,
            2,
            "id",
            Message::new(
                "two",
                Body::Ping(Ping {
                    purpose: "connection_test".into(),
                }),
            ),
        );
        state.closed(2);
        assert_eq!(state.status(late).phase, "failed");
    }
}
