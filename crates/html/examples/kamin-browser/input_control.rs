//! Finite input series through GPUI, independent of the desktop's physical pointer.

use gpui::{AsyncWindowContext, MouseMoveEvent, PlatformInput, Task, point, px};
use kamin_html::why::WindowJournal;
use serde_json::{Value, json};
use std::{error::Error, path::PathBuf, time::Duration};

pub struct MouseSeries {
    pub count: u32,
    pub interval: Duration,
    x: f32,
    y: f32,
}

impl MouseSeries {
    pub fn parse(value: &Value) -> Result<Self, Box<dyn Error>> {
        let count = value["count"].as_u64().ok_or("input count is required")?;
        let interval = value["interval_ms"]
            .as_u64()
            .ok_or("input interval_ms is required")?;
        if !(1..=600).contains(&count)
            || !(1..=1000).contains(&interval)
            || count * interval > 60_000
        {
            return Err("input series must contain 1..600 events within 60 seconds".into());
        }
        let coordinate = |name: &str| -> Result<f32, Box<dyn Error>> {
            let value = value[name].as_f64().ok_or("input coordinate is required")?;
            if !value.is_finite() || !(0.0..=100_000.0).contains(&value) {
                return Err("input coordinates must be finite nonnegative CSS pixels".into());
            }
            Ok(value as f32)
        };
        Ok(Self {
            count: count as u32,
            interval: Duration::from_millis(interval),
            x: coordinate("x")?,
            y: coordinate("y")?,
        })
    }

    pub fn event(&self, index: u32) -> PlatformInput {
        PlatformInput::MouseMove(MouseMoveEvent {
            position: point(px(self.x + (index % 7) as f32), px(self.y)),
            pressed_button: None,
            modifiers: Default::default(),
        })
    }
}

pub fn start(
    value: &Value,
    reply: PathBuf,
    journal: WindowJournal,
    cx: &mut AsyncWindowContext,
) -> Result<Task<()>, Box<dyn Error>> {
    let id = value["id"]
        .as_str()
        .ok_or("input command has no id")?
        .to_owned();
    if value["op"] != "mouse" {
        return Err("unknown input operation".into());
    }
    let series = MouseSeries::parse(value)?;
    // Reject the complete path before dispatching its first event.
    cx.update(|window, _| {
        let size = window.viewport_size();
        series.x + 6.0 < f32::from(size.width) && series.y < f32::from(size.height)
    })?
    .then_some(())
    .ok_or("input path lies outside the window")?;
    let write_reply = move |state: &str| {
        std::fs::write(
            &reply,
            json!({"schema": 1, "id": id, "state": state}).to_string(),
        )
    };
    write_reply("running")?;
    Ok(cx.spawn(async move |cx| {
        for index in 0..series.count {
            if index > 0 {
                cx.background_executor().timer(series.interval).await;
            }
            if let Err(error) = cx.update(|window, cx| {
                window.dispatch_event(series.event(index), cx);
            }) {
                journal.error(format!("Input series: {error}"));
                let _ = write_reply("failed");
                return;
            }
        }
        if let Err(error) = write_reply("complete") {
            journal.error(format!("Input series reply: {error}"));
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_series_rejects_unbounded_or_invalid_input() {
        for value in [
            json!({"count":0,"interval_ms":100,"x":700,"y":400}),
            json!({"count":601,"interval_ms":100,"x":700,"y":400}),
            json!({"count":100,"interval_ms":1000,"x":700,"y":400}),
            json!({"count":10,"interval_ms":0,"x":700,"y":400}),
            json!({"count":10,"interval_ms":100,"x":-1,"y":400}),
        ] {
            assert!(MouseSeries::parse(&value).is_err());
        }
        assert!(
            MouseSeries::parse(&json!({
                "count":160,"interval_ms":100,"x":700,"y":400
            }))
            .is_ok()
        );
    }
}
