//! Output-каналы (output-channels.ts 1:1): буферы per `${extId}::${name}`,
//! события kamin:output:event (append/replace/clear/dispose/show), кап
//! 256КБ → трим до хвоста 192КБ. Плюс system-log (диагностика хоста/шелла).

const KB: usize = 1024;
const MAX_CHANNEL_BYTES: usize = 256 * KB;
const TRIM_TARGET_BYTES: usize = 192 * KB;

#[derive(Clone, Debug)]
pub struct OutputChannel {
    /// `${extensionId}::${name}`.
    pub key: String,
    pub extension_id: String,
    pub name: String,
    pub buffer: String,
}

#[derive(Default)]
pub struct OutputChannels {
    pub channels: Vec<OutputChannel>,
    pub active: Option<String>,
}

impl OutputChannels {
    /// Применить событие; true → op=="show" (нужно открыть Customize→Logs).
    pub fn apply(
        &mut self,
        extension_id: &str,
        channel: &str,
        op: &str,
        text: Option<&str>,
    ) -> bool {
        let k = format!("{extension_id}::{channel}");
        if op == "dispose" {
            self.channels.retain(|c| c.key != k);
            if self.active.as_deref() == Some(k.as_str()) {
                self.active = None;
            }
            return false;
        }
        if !self.channels.iter().any(|c| c.key == k) {
            self.channels.push(OutputChannel {
                key: k.clone(),
                extension_id: extension_id.to_string(),
                name: channel.to_string(),
                buffer: String::new(),
            });
        }
        if op == "show" {
            self.active = Some(k);
            return true;
        }
        if self.active.is_none() {
            self.active = Some(k.clone());
        }
        let Some(c) = self.channels.iter_mut().find(|c| c.key == k) else {
            return false;
        };
        match op {
            "append" => {
                c.buffer.push_str(text.unwrap_or(""));
                c.buffer = tail(&c.buffer);
            }
            "replace" => c.buffer = tail(text.unwrap_or("")),
            _ => c.buffer.clear(), // clear
        }
        false
    }

    pub fn clear(&mut self, key: &str) {
        if let Some(c) = self.channels.iter_mut().find(|c| c.key == key) {
            c.buffer.clear();
        }
    }

    pub fn active_channel(&self) -> Option<&OutputChannel> {
        let k = self.active.as_deref()?;
        self.channels.iter().find(|c| c.key == k)
    }
}

/// Кап буфера: byte-safe трим до хвоста (по границе символа).
fn tail(buf: &str) -> String {
    if buf.len() <= MAX_CHANNEL_BYTES {
        return buf.to_string();
    }
    let cut = buf.len() - TRIM_TARGET_BYTES;
    let mut i = cut;
    while i < buf.len() && !buf.is_char_boundary(i) {
        i += 1;
    }
    buf[i..].to_string()
}

/// Запись system-лога (system-log.ts): level + источник + сообщение + время.
#[derive(Clone, Debug)]
pub struct SysEntry {
    pub level: &'static str, // error|warning|info
    pub source: String,
    pub message: String,
    pub at: std::time::SystemTime,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_creates_and_activates() {
        let mut o = OutputChannels::default();
        o.apply("ext.a", "Log", "append", Some("hello\n"));
        assert_eq!(o.channels.len(), 1);
        assert_eq!(o.active.as_deref(), Some("ext.a::Log"));
        assert_eq!(o.active_channel().unwrap().buffer, "hello\n");
    }

    #[test]
    fn show_switches_active_and_signals() {
        let mut o = OutputChannels::default();
        o.apply("a", "One", "append", Some("x"));
        let show = o.apply("b", "Two", "show", None);
        assert!(show);
        assert_eq!(o.active.as_deref(), Some("b::Two"));
    }

    #[test]
    fn dispose_removes() {
        let mut o = OutputChannels::default();
        o.apply("a", "One", "append", Some("x"));
        o.apply("a", "One", "dispose", None);
        assert!(o.channels.is_empty());
        assert_eq!(o.active, None);
    }

    #[test]
    fn cap_trims_to_tail() {
        let mut o = OutputChannels::default();
        let chunk = "y".repeat(300 * KB);
        o.apply("a", "Big", "append", Some(&chunk));
        let len = o.active_channel().unwrap().buffer.len();
        assert!(len <= TRIM_TARGET_BYTES);
    }
}
