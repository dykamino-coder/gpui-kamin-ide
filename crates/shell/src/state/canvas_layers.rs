//! Служебные слои главного окна: слежение за темой ОС и фон с «дырой»
//! под composition-вебвью.
//!
//! Куски цепочки `render` перенесены как есть (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::root::system_theme;
use crate::state::model::RootView;
use gpui::Context;
use gpui::prelude::*;
use kamin_theme::Palette;

impl RootView {
    pub(crate) fn with_canvas_layers<E>(
        &mut self,
        root: E,
        p: &'static Palette,
        cx: &mut Context<Self>,
    ) -> E
    where
        E: gpui::ParentElement,
    {
        root.child({
            // System-выбор: следовать OS-теме на лету (apply только при
            // реальной смене — иначе рекурсивные refresh)
            if self.theme_choice == "system" && self.contrib_theme_id.is_none() {
                let k = system_theme(cx);
                if k != self.theme {
                    self.theme = k;
                    crate::theme_sync::apply(k, cx);
                }
            }
            gpui::Empty
        })
        .child({
            // Корневой фон с «дырой» под composition-вебвью (обычный
            // .bg() непрозрачно закрасил бы зону над underlay-визуалом)
            let bgc = rgba(p.bg_sidebar);
            gpui::canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    // Прогрев иконок В ПЕРВЫЕ кадры, ПОД фоном (см. warm_all):
                    // поздние аллокации атласа затаптывались CEF.
                    crate::icon_raster::warm_all(window);
                    // В режиме CEF фон рисуется целиком: вырезать не под что.
                    // Дыр под страницы больше нет: CEF рисует их прямо в кадр.
                    #[cfg(windows)]
                    let holes: Vec<[f32; 4]> = Vec::new();
                    #[cfg(not(windows))]
                    let holes: Vec<[f32; 4]> = Vec::new();
                    let dbg = false /* KAMIN_VWV_PAINTDBG законстанчен OFF (диагностика plan/97) */;
                    let col = if dbg {
                        gpui::Rgba {
                            r: 1.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }
                    } else {
                        bgc
                    };
                    for seg in crate::ui::glint::hole_segments_multi(bounds, &holes) {
                        window.paint_quad(gpui::fill(seg, col));
                    }
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full()
        })
    }
}
