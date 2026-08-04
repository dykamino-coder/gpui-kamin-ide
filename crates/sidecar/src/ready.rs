//! Парсинг ready-кадра хоста (host-main.ts:156):
//! {kind:"evt", channel:"kamin-host:ready", payload:{extensions, wsPort, wsToken}}

use serde::Serialize;

const READY_CHANNEL: &str = "kamin-host:ready";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct HostEndpoint {
    pub port: u16,
    pub token: String,
}

/// Одна stdout-строка → endpoint, если это ready-событие.
/// Не-кадры (логи активации и пр.) → None.
pub fn parse_ready(line: &str) -> Option<HostEndpoint> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if value.get("kind")?.as_str()? != "evt" {
        return None;
    }
    if value.get("channel")?.as_str()? != READY_CHANNEL {
        return None;
    }
    let payload = value.get("payload")?;
    let port = u16::try_from(payload.get("wsPort")?.as_u64()?).ok()?;
    let token = payload.get("wsToken")?.as_str()?.to_owned();
    Some(HostEndpoint { port, token })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_ready_frame() {
        let line = r#"{"kind":"evt","channel":"kamin-host:ready","payload":{"extensions":0,"wsPort":54321,"wsToken":"abcdef"}}"#;
        let ep = parse_ready(line).unwrap();
        assert_eq!(ep.port, 54321);
        assert_eq!(ep.token, "abcdef");
    }

    #[test]
    fn skips_non_frames_and_other_events() {
        assert!(parse_ready("activation: claude-bridge ok").is_none());
        assert!(
            parse_ready(r#"{"kind":"evt","channel":"kamin-host:fatal","payload":{}}"#).is_none()
        );
        assert!(parse_ready(r#"{"kind":"res","id":1,"ok":true}"#).is_none());
    }
}
