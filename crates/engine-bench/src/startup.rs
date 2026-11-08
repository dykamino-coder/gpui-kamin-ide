//! Последовательные интервалы запуска до первого GPU-кадра, отдельно от документа.

use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

pub(super) struct Startup {
    previous: Instant,
    stages: BTreeMap<&'static str, f64>,
    /// Конец принудительных презентов платформы после последней смены
    /// размера окна (`gpui_windows::RESIZE_PRESENTATION_BOOST`).
    presentation_settles: Option<Instant>,
}

impl Startup {
    pub(super) fn new() -> Self {
        Self {
            previous: Instant::now(),
            stages: BTreeMap::new(),
            presentation_settles: None,
        }
    }

    pub(super) fn mark(&mut self, name: &'static str) {
        let now = Instant::now();
        self.stages
            .insert(name, (now - self.previous).as_secs_f64() * 1000.);
        self.previous = now;
    }

    /// Окно только что сменило размер или рамку: до конца буста каждый
    /// WM_PAINT (vsync) презентует неизменную сцену и даёт захват кадра.
    pub(super) fn size_changed(&mut self) {
        self.presentation_settles = Some(Instant::now() + gpui_windows::RESIZE_PRESENTATION_BOOST);
    }

    /// Ждёт конца буста: дальше окно не получает непрошеных кадров, и ни
    /// образцы быстрых страниц, ни окно аудита памяти не видят презентов.
    pub(super) async fn settle(&mut self, cx: &gpui::AsyncApp) {
        let remaining = self.presentation_settles.map_or(Duration::ZERO, |at| {
            at.saturating_duration_since(Instant::now())
        });
        cx.background_executor().timer(remaining).await;
        self.mark("resize_presentation_boost");
    }

    pub(super) fn report(&self) -> serde_json::Value {
        serde_json::json!({"stages_ms": self.stages,
            "total_ms": self.stages.values().sum::<f64>(),
            "intervals_overlap": false,
            "scope": "host launch through initial empty GPU readback and the platform's post-resize presentation window; excludes executable loader and CLI parsing"})
    }
}
