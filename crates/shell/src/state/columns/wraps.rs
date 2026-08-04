//! Обёртки колонок и ручки сплиттеров между ними.
//!
//! Куски `render` перенесены как есть (`plan/100-refactor-250.md`).

use crate::root::DragKind;
use crate::state::model::RootView;
use crate::ui::splitter::v_handle;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

/// Готовые элементы строки тела в порядке слева направо.
pub(crate) struct Wraps {
    pub main_wrap: AnyElement,
    pub main_file_handle: AnyElement,
    pub file_wrap: AnyElement,
    pub file_right_handle: AnyElement,
    pub right_wrap: AnyElement,
    pub main_column_present: bool,
    pub file_fills: bool,
    pub right_fills: bool,
}

impl RootView {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn column_wraps(
        &mut self,
        cx: &mut Context<Self>,
        p: &'static Palette,
        main_column: AnyElement,
        file_column: AnyElement,
        right_column_el: AnyElement,
        _sidebar_w: f32,
        file_w: f32,
        right_w: f32,
        _no_sessions: bool,
    ) -> Wraps {
        let main_wrap: AnyElement = div()
            .flex_1()
            .min_w(px(m::PANEL_MIN_SIZE))
            .h_full()
            .child(main_column)
            .into_any_element();
        let main_file_handle: AnyElement = v_handle(
            "main-file-handle",
            p,
            self.handle_show("main-file-handle", DragKind::MainFile),
            self.drag.is_some(),
            cx.listener(move |this, e: &gpui::MouseDownEvent, window, cx| {
                let vw = f32::from(window.viewport_size().width);
                this.begin_drag(DragKind::MainFile, f32::from(e.position.x), vw);
                cx.notify();
            }),
            cx.listener(move |this, hovered: &bool, _, cx| {
                this.hovered_handle = if *hovered {
                    Some("main-file-handle")
                } else {
                    None
                };
                cx.notify();
            }),
        )
        .into_any_element();
        // `fileFills` — main скрыт и File видна: колонка тянется на всё
        // освободившееся место вместо фиксированной ширины
        let main_column_present = self.cz.customize_open || self.layout.main_visible;
        let file_fills = !main_column_present && self.layout.file_panel_visible;
        let right_fills = !main_column_present && !self.layout.file_panel_visible;
        let mut file_wrap_el = div().h_full().min_w(px(m::PANEL_MIN_SIZE));
        file_wrap_el = if file_fills {
            file_wrap_el.flex_1()
        } else {
            // `.filePanel { flex-shrink: 1 }` + `minWidth: 100`.
            // Сохранённое число — ширина КАРТЫ, а `gap_wrap` съедает по 4 с
            // каждой стороны: прибавляем их к обёртке, иначе карта на 8px
            // уже сохранённой (тот же приём, что у сайдбара, ревью ц.13)
            file_wrap_el.w(px(file_w + 8.0))
        };
        let file_wrap: AnyElement = file_wrap_el.child(file_column).into_any_element();
        let file_right_handle: AnyElement = v_handle(
            "file-right-handle",
            p,
            self.handle_show("file-right-handle", DragKind::FileRight),
            self.drag.is_some(),
            cx.listener(move |this, e: &gpui::MouseDownEvent, window, cx| {
                let vw = f32::from(window.viewport_size().width);
                this.begin_drag(DragKind::FileRight, f32::from(e.position.x), vw);
                cx.notify();
            }),
            cx.listener(move |this, hovered: &bool, _, cx| {
                this.hovered_handle = if *hovered {
                    Some("file-right-handle")
                } else {
                    None
                };
                cx.notify();
            }),
        )
        .into_any_element();
        // width = persisted ВКЛЮЧАЯ rail (оригинал; ревью ц.1: был +rail)
        let mut right_wrap_el = div().h_full().min_w(px(m::PANEL_MIN_SIZE));
        right_wrap_el = if right_fills {
            right_wrap_el.flex_1()
        } else {
            right_wrap_el // `gap_wrap` правой колонки съедает 4 слева (справа карта вплотную
                // к рейлу) — компенсируем, чтобы ширина карты равнялась
                // сохранённой (ревью ц.13)
                .w(px(right_w + 4.0))
        };
        let right_wrap: AnyElement = right_wrap_el.child(right_column_el).into_any_element();
        Wraps {
            main_wrap,
            main_file_handle,
            file_wrap,
            file_right_handle,
            right_wrap,
            main_column_present,
            file_fills,
            right_fills,
        }
    }
}
