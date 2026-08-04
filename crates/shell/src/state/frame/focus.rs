//! Фокус кадра: шаг табуляции, замер строки для probe, кольца фокуса.
//!
//! Кусок `render` вынесен как есть (`plan/100-refactor-250.md`): порядок вызовов в кадре прежний.

use crate::state::consts::UI_FONT;
use crate::state::model::RootView;
use gpui::{Context, Window, px};

impl RootView {
    pub(crate) fn frame_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // `:focus-visible`: снимок фокуса на кадр + чеканка новых хэндлов
        // (рисующие функции не получают ни `cx`, ни `window`)
        if let Some(forward) = self.pending_focus_step.take() {
            if forward {
                window.focus_next();
            } else {
                window.focus_prev();
            }
        }
        // probe `shape`: считаем ширину строки ЭТИМ ЖЕ шейпером, что и рендер
        if let Some(req) = crate::probe::registry::take_shape_request() {
            let font = gpui::Font {
                family: if req.mono {
                    "JetBrains Mono".into()
                } else {
                    UI_FONT.into()
                },
                features: gpui::FontFeatures::default(),
                fallbacks: None,
                weight: gpui::FontWeight(req.weight),
                style: gpui::FontStyle::Normal,
            };
            let run = gpui::TextRun {
                len: req.text.len(),
                font,
                color: gpui::black(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let w = f32::from(
                window
                    .text_system()
                    .shape_line_spaced(
                        gpui::SharedString::from(req.text.clone()),
                        px(req.size),
                        &[run],
                        None,
                        px(req.spacing),
                    )
                    .width,
            );
            crate::probe::registry::record_shape(req.text, w);
        }
        crate::ui::focus_ring::begin_frame(window);
        if crate::ui::focus_ring::flush(cx) {
            cx.notify();
        }
    }
}
