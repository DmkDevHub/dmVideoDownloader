use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub const MAX_BYTES: usize = 65_536;
pub const HOST_NAME: &str = "com.browser_video_downloader.host";
pub const HOST_EXE: &str = "VideoNativeHost.exe";
pub const MANIFEST: &str = "native-host-manifest.json";

pub fn extension_id(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|byte| (b'a'..=b'p').contains(&byte))
}
pub fn extension_origin(value: &str) -> Option<&str> {
    value
        .strip_prefix("chrome-extension://")?
        .strip_suffix('/')
        .filter(|id| extension_id(id))
}
fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Code {
    InvalidPayload,
    UnsupportedVersion,
    UrlNotAllowed,
    HostNotRegistered,
    HostStartFailed,
    HostPathMismatch,
    ExtensionIdMismatch,
    AppNotRunning,
    AppLaunchFailed,
    IpcTimeout,
    ConnectionTestTimeout,
    ConnectionTestNotWaiting,
    DownloadEngineUnavailable,
    RegistrationFailed,
}
impl Code {
    pub fn message(self) -> &'static str {
        match self {
            Self::InvalidPayload => "메시지 형식 또는 요청 상태를 확인하세요.",
            Self::UnsupportedVersion => "앱과 확장의 프로토콜 버전을 확인하세요.",
            Self::UrlNotAllowed => "지원하지 않는 영상 주소입니다.",
            Self::HostNotRegistered => "앱에서 호스트를 등록하세요.",
            Self::HostStartFailed => "호스트와 연결하지 못했습니다.",
            Self::HostPathMismatch => "현재 앱 위치에서 호스트를 다시 등록하세요.",
            Self::ExtensionIdMismatch => "Chrome의 실제 확장 ID를 확인하세요.",
            Self::AppNotRunning => "Windows 앱을 실행한 뒤 다시 시도하세요.",
            Self::AppLaunchFailed => "Windows 앱을 시작할 수 없습니다.",
            Self::IpcTimeout => "앱 응답 시간이 초과됐습니다.",
            Self::ConnectionTestTimeout => "연결 테스트 시간이 초과됐습니다.",
            Self::ConnectionTestNotWaiting => "앱에서 연결 테스트 대기를 먼저 시작하세요.",
            Self::DownloadEngineUnavailable => "다운로드 도구를 확인하세요.",
            Self::RegistrationFailed => "호스트 등록 상태를 확인하세요.",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    Mp4,
    Webm,
    Hls,
    Dash,
    Unknown,
}

macro_rules! payload {
    ($name:ident { $($fields:tt)* }) => {
        #[derive(Clone, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        pub struct $name { $($fields)* }
    };
}
payload!(Candidate {
    pub candidate_id: String, pub tab_id: u32, pub page_url: String,
    #[serde(skip_serializing_if = "Option::is_none")] pub page_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub media_url: Option<String>,
    pub source_type: SourceType, pub detected_at: String,
});
payload!(Ping { pub purpose: String });
payload!(Pong { pub protocol_version: u32, pub app_reachable: bool });
payload!(TestResult { pub success: bool });
payload!(Ack { pub accepted: bool, #[serde(skip_serializing_if = "Option::is_none")] pub candidate_id: Option<String> });
payload!(Empty {});
payload!(RuntimeStatus { pub protocol_version: u32, pub app_reachable: bool, pub browser_auto_detect: bool });
payload!(Error { pub code: Code, pub safe_message: String });

#[derive(Clone, Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Body {
    VideoCandidate(Candidate),
    Ping(Ping),
    Pong(Pong),
    ConnectionTestResult(TestResult),
    Ack(Ack),
    GetRuntimeStatus(Empty),
    RuntimeStatus(RuntimeStatus),
    Error(Error),
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    version: u32,
    pub request_id: String,
    #[serde(flatten)]
    pub body: Body,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Wire {
    version: u32,
    request_id: String,
    #[serde(rename = "type")]
    kind: String,
    payload: Box<RawValue>,
}
impl Message {
    pub fn new(request_id: impl Into<String>, body: Body) -> Self {
        Self {
            version: 1,
            request_id: request_id.into(),
            body,
        }
    }
    pub fn error(request_id: impl Into<String>, code: Code) -> Self {
        Self::new(
            request_id,
            Body::Error(Error {
                code,
                safe_message: code.message().into(),
            }),
        )
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, Code> {
        if bytes.len() > MAX_BYTES {
            return Err(Code::InvalidPayload);
        }
        let wire: Wire = serde_json::from_slice(bytes).map_err(|_| Code::InvalidPayload)?;
        if wire.version != 1 {
            return Err(Code::UnsupportedVersion);
        }
        macro_rules! body {
            ($variant:ident) => {
                Body::$variant(
                    serde_json::from_str(wire.payload.get()).map_err(|_| Code::InvalidPayload)?,
                )
            };
        }
        let body = match wire.kind.as_str() {
            "VIDEO_CANDIDATE" => body!(VideoCandidate),
            "PING" => body!(Ping),
            "PONG" => body!(Pong),
            "CONNECTION_TEST_RESULT" => body!(ConnectionTestResult),
            "ACK" => body!(Ack),
            "GET_RUNTIME_STATUS" => body!(GetRuntimeStatus),
            "RUNTIME_STATUS" => body!(RuntimeStatus),
            "ERROR" => body!(Error),
            _ => return Err(Code::InvalidPayload),
        };
        let message = Self::new(wire.request_id, body);
        message.validate()?;
        Ok(message)
    }
    pub fn encode(&self) -> Result<Vec<u8>, Code> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| Code::InvalidPayload)?;
        if bytes.len() > MAX_BYTES {
            return Err(Code::InvalidPayload);
        }
        Ok(bytes)
    }
    fn validate(&self) -> Result<(), Code> {
        if !valid_id(&self.request_id) {
            return Err(Code::InvalidPayload);
        }
        match &self.body {
            Body::VideoCandidate(c) => {
                if !valid_id(&c.candidate_id)
                    || c.tab_id > i32::MAX as u32
                    || c.page_title.as_ref().is_some_and(|title| {
                        title.chars().count() > 200 || title.chars().any(char::is_control)
                    })
                    || OffsetDateTime::parse(&c.detected_at, &Rfc3339).is_err()
                {
                    return Err(Code::InvalidPayload);
                }
                if !public_url(&c.page_url)
                    || c.media_url.as_deref().is_some_and(|url| !public_url(url))
                {
                    return Err(Code::UrlNotAllowed);
                }
            }
            Body::Ping(p) if p.purpose != "connection_test" => return Err(Code::InvalidPayload),
            Body::Pong(p) if p.protocol_version != 1 => return Err(Code::UnsupportedVersion),
            Body::RuntimeStatus(p) if p.protocol_version != 1 => {
                return Err(Code::UnsupportedVersion)
            }
            Body::ConnectionTestResult(p) if !p.success => return Err(Code::InvalidPayload),
            Body::Ack(p) if p.candidate_id.as_deref().is_some_and(|id| !valid_id(id)) => {
                return Err(Code::InvalidPayload)
            }
            Body::Error(p)
                if p.safe_message.chars().count() > 256
                    || p.safe_message.chars().any(char::is_control) =>
            {
                return Err(Code::InvalidPayload)
            }
            _ => {}
        }
        Ok(())
    }
}

pub fn public_url(value: &str) -> bool {
    use url::Host;
    if value.len() > 8192 || value.bytes().any(|b| b <= 32 || b == 127) {
        return false;
    }
    let Ok(url) = url::Url::parse(value) else {
        return false;
    };
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return false;
    }
    let allowed_host = match url.host() {
        Some(Host::Domain(domain)) => {
            let domain = domain.trim_end_matches('.');
            domain.contains('.')
                && !matches!(
                    domain.rsplit('.').next(),
                    Some("localhost" | "local" | "internal" | "lan" | "home" | "invalid")
                )
        }
        Some(Host::Ipv4(ip)) => {
            let [a, b, _, _] = ip.octets();
            a != 0
                && a < 224
                && !ip.is_private()
                && !ip.is_loopback()
                && !ip.is_link_local()
                && !(a == 100 && (64..=127).contains(&b)
                    || a == 192 && b == 0
                    || a == 198 && matches!(b, 18 | 19))
        }
        Some(Host::Ipv6(ip)) => {
            let [a, b, _, _, _, _, _, _] = ip.segments();
            a & 0xe000 == 0x2000 && !(a == 0x2001 && b == 0x0db8)
        }
        None => false,
    };
    if !allowed_host {
        return false;
    }
    if url.path().split('/').any(|part| {
        matches!(
            part.to_ascii_lowercase().as_str(),
            "token" | "auth" | "session" | "bearer"
        )
    }) {
        return false;
    }
    !url.query_pairs().any(|(key, value)| {
        let key: String = key
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .map(|ch| ch.to_ascii_lowercase())
            .collect();
        [
            "token",
            "signature",
            "password",
            "passwd",
            "secret",
            "credential",
            "authorization",
            "session",
            "jwt",
            "xamz",
            "xgoog",
        ]
        .iter()
        .any(|part| key.contains(part))
            || matches!(
                key.as_str(),
                "auth" | "sig" | "key" | "apikey" | "policy" | "hdnea" | "hdnts"
            )
            || value.to_ascii_lowercase().starts_with("bearer ")
            || (value.split('.').count() == 3
                && value.split('.').all(|part| {
                    !part.is_empty()
                        && part
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
                }))
    })
}

pub fn utc_now() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_wire_fixtures() {
        let cases: Vec<(String, Option<Code>)> =
            serde_json::from_str(include_str!("../../protocol-fixtures.json")).unwrap();
        for (index, (wire, expected)) in cases.into_iter().enumerate() {
            let decoded = Message::decode(wire.as_bytes());
            assert_eq!(decoded.as_ref().err().copied(), expected, "fixture {index}");
            if let Ok(message) = decoded {
                assert!(Message::decode(&message.encode().unwrap()).is_ok());
            }
        }
        assert!(matches!(Message::decode(&[255]), Err(Code::InvalidPayload)));
        assert!(matches!(
            Message::decode(&vec![b' '; MAX_BYTES + 1]),
            Err(Code::InvalidPayload)
        ));
    }
}

pub mod frame;

pub fn request_id_hint(bytes: &[u8]) -> Option<String> {
    let wire: Wire = serde_json::from_slice(bytes).ok()?;
    valid_id(&wire.request_id).then_some(wire.request_id)
}
