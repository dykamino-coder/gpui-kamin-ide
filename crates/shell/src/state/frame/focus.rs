//! Фокус кадра: шаг табуляции, замер строки для probe, кольца фокуса.
//!
//! Кусок `render` вынесен как есть (`plan/100-refactor-250.md`): порядок вызовов в кадре прежний.

use crate::state::consts::UI_FONT;
use crate::state::model::RootView;
use gpui::{Context, Window, px};

impl RootView {
    pub(crate) fn frame_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Апдейт готов к установке: закрываем окно ШТАТНО — main() после
        // цикла сделает web::shutdown(), CEF-кэш останется чистым.
        if self.pending_quit {
            self.pending_quit = false;
            // Последний лейаут активной сессии — синхронно, до убийства хоста
            // Job'ом (иначе «лейаут не тот, каким оставил» после перезагрузки).
            self.persist_active_session_layout_sync();
            window.remove_window();
            return;
        }
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
                // Ширина начертания у шрифтов оболочки обычная.
                stretch: gpui::FontStretch::Normal,
            };
            let run = gpui::TextRun {
                len: req.text.len(),
                font,
                color: gpui::black(),
                background_color: None,
                // Поля строчного бокса вокруг фона прогона: здесь их нет.
                font_size: None,
                background_pad: Default::default(),
                background_radius: Default::default(),
                background_border: None,
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
