//! On-screen profiler: frames, CPU, memory and GPU of the viewer, refreshed once a second.
//! Frame numbers come from the engine's own journal; the panel adds no meter of its own.

#[cfg(feature = "why-profiler")]
#[path = "hud/frames.rs"]
mod frames;
#[path = "hud/gpu.rs"]
mod gpu;
#[path = "hud/process.rs"]
mod process;

use crate::{app::Browser, viewport};
use gpui::{App, Context, IntoElement, SharedString, Task, Window, div, prelude::*, px, rgb, rgba};
use std::time::{Duration, Instant};

pub struct Hud {
    pub visible: bool,
    lines: Vec<SharedString>,
    last: Option<(Instant, process::Sample)>,
    gpu: Option<gpu::Counters>,
    cores: f64,
    #[cfg(feature = "why-profiler")]
    frames: frames::Frames,
}

fn mb(bytes: u64) -> f64 {
    bytes as f64 / (1024. * 1024.)
}

impl Hud {
    pub fn new(window: &Window, cx: &App) -> Self {
        #[cfg(not(feature = "why-profiler"))]
        let _ = (window, cx);
        Self {
            visible: true,
            lines: vec!["Профилировщик: первая выборка через секунду".into()],
            last: None,
            gpu: gpu::Counters::open(),
            cores: std::thread::available_parallelism().map_or(1., |n| n.get() as f64),
            #[cfg(feature = "why-profiler")]
            frames: frames::Frames::new(window, cx),
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        // A hidden panel takes no samples; the next one starts a fresh interval.
        self.last = None;
        self.lines = vec!["Профилировщик: первая выборка через секунду".into()];
    }

    fn sample(&mut self, viewport: (f32, f32), window: &Window, cx: &App) {
        let now = Instant::now();
        let process = process::sample();
        let elapsed = self
            .last
            .map(|(at, _)| now.duration_since(at).as_secs_f64())
            .filter(|seconds| *seconds > 0.);
        let mut lines = Vec::new();
        #[cfg(feature = "why-profiler")]
        {
            let frames = self.frames.drain();
            let fps = elapsed.map_or(0., |seconds| f64::from(frames.presented) / seconds);
            lines.push(format!(
                "КАДРЫ   {fps:.0} к/с   интервал ср/макс {} мс",
                frames.interval.text()
            ));
            lines.push(format!(
                "        отрисовка {} мс   выдача {} мс",
                frames.draw.text(),
                frames.present.text()
            ));
            lines.push(format!(
                "        от правки до экрана {} мс   пропущено {}   потеряно записей {}",
                frames.dirty_to_present.text(),
                frames.skipped,
                frames.lost
            ));
        }
        #[cfg(not(feature = "why-profiler"))]
        lines.push("КАДРЫ   сборка без журнала движка (why-profiler)".to_owned());
        match (process, self.last, elapsed) {
            (Some(process), Some((_, before)), Some(seconds)) => {
                let cpu = process.cpu_100ns.saturating_sub(before.cpu_100ns) as f64 / 1e7 / seconds;
                lines.push(format!(
                    "CPU     {:.1} % одного ядра   {:.1} % машины ({:.0} лог. ядер)",
                    cpu * 100.,
                    cpu * 100. / self.cores,
                    self.cores
                ));
            }
            _ => lines.push("CPU     -".to_owned()),
        }
        if let Some(process) = process {
            lines.push(format!(
                "ПАМЯТЬ  частная {:.0} МБ   рабочий набор {:.0} МБ (пик {:.0})",
                mb(process.private),
                mb(process.working),
                mb(process.peak_working)
            ));
        }
        let gpu = self
            .gpu
            .as_ref()
            .map(gpu::Counters::sample)
            .unwrap_or_default();
        let value = |bytes: Option<u64>| bytes.map_or("-".to_owned(), |b| format!("{:.0}", mb(b)));
        lines.push(format!(
            "GPU     загрузка {} %   память выделенная {} МБ, общая {} МБ",
            gpu.load.map_or("-".to_owned(), |load| format!("{load:.0}")),
            value(gpu.dedicated),
            value(gpu.shared)
        ));
        let mut textures = 0;
        window.gpu_memory(&mut |_, bytes, _| textures += bytes);
        let device = window.gpu_specs().map_or(
            "устройство неизвестно".to_owned(),
            |specs| {
                if specs.is_software_emulated {
                    format!("{} (программный растр)", specs.device_name)
                } else {
                    specs.device_name
                }
            },
        );
        lines.push(format!(
            "        текстуры движка {:.1} МБ   {device}",
            mb(textures)
        ));
        lines.push(format!(
            "ОКНО    страница {:.0} x {:.0}   масштаб {:.2}",
            viewport.0,
            viewport.1,
            window.scale_factor()
        ));
        lines.push("F3 - скрыть. Панель сама рисует 1 кадр/с.".to_owned());
        self.lines = lines.into_iter().map(SharedString::from).collect();
        if let Some(process) = process {
            self.last = Some((now, process));
        }
    }

    pub fn panel(&self) -> impl IntoElement {
        let mut panel = div()
            .absolute()
            .top(px(viewport::TOOLBAR_HEIGHT + 8.))
            .right(px(24.))
            .flex()
            .flex_col()
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(rgba(0x000000d8))
            .text_color(rgb(0x8dff9f))
            .text_size(px(12.))
            .font_family("Consolas")
            .whitespace_nowrap();
        for line in &self.lines {
            panel = panel.child(line.clone());
        }
        panel
    }
}

/// One wake-up a second while the panel is shown; a hidden panel wakes and leaves.
pub fn start(window: &mut Window, cx: &mut Context<Browser>) -> Task<()> {
    cx.spawn_in(window, async move |this, cx| {
        loop {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let alive = this.update_in(cx, |this, window, cx| {
                if this.hud.visible {
                    let viewport = this.viewport;
                    this.hud.sample(viewport, window, cx);
                    cx.notify();
                }
            });
            if alive.is_err() {
                break;
            }
        }
    })
}
