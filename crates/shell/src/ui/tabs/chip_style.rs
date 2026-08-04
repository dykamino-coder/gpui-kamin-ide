//! Оформление чипа сессии: фон, рамка, градиент активного, ховер.
//!
//! Блок перенесён как есть (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use gpui::prelude::*;
use gpui::{SharedString, div, linear_color_stop, linear_gradient, px};
use kamin_metrics as m;
use kamin_model::Session;
use kamin_theme::Palette;

#[allow(clippy::too_many_arguments)]
pub(crate) fn chip_style(
    s: &Session,
    is_active: bool,
    has_color: bool,
    is_light: bool,
    tab_color: gpui::Rgba,
    first: bool,
    chip_w: f32,
    p: &Palette,
) -> gpui::Stateful<gpui::Div> {
    let mut tab = div()
        .id(SharedString::from(format!("tab-{}", s.id)))
        .occlude() // титлбар = drag-область; чип ловит клики сам
        .relative()
        // Поэлементный кроп в parity/shots.py: регион у ПЕРВОГО чипа
        .when(first, |d| {
            d.child(crate::probe::registry::probe_area("session-chip"))
        })
        .flex()
        .items_center()
        .gap(px(6.0))
        .pl(px(10.0))
        .pr(px(6.0))
        // first-child ml 6, остальные 2 (SessionTab.module.css)
        .ml(px(if first { 6.0 } else { 2.0 }))
        .mr(px(1.0))
        .h(px(28.0))
        // Ширину считаем САМИ (см. session_tabs): flex-shrink давал
        // недетерминированную итоговую ширину, из-за чего усечение текста
        // не совпадало с реальным клипом. Диапазон тот же: 44..180(240).
        .w(px(chip_w))
        .flex_shrink_0()
        .rounded(px(m::RADIUS_MD))
        .overflow_hidden()
        .cursor_pointer()
        .text_size(px(12.0))
        .text_color(rgba(p.text_secondary))
        // border-резерв 1px transparent ВСЕГДА (иначе сдвиг контента у active)
        .border_1()
        .border_color(tint(rgba(p.text_primary), 0.0))
        .bg(rgba(p.bg_mantle));

    // Каскад оригинала: `.active` (`SessionTab.module.css:29-34`) объявлен
    // ПЕРЕД `.tinted` (`:37`) при равной специфичности (0,1,0), поэтому у
    // ЦВЕТНОЙ и одновременно активной сессии в ТЁМНОЙ теме побеждает
    // `.tinted`. В светлой теме `[data-theme=light] .active` (0,2,0) идёт
    // после light-`.tinted` и выигрывает. Мы рисовали active всегда — на
    // цветных активных сессиях фон был темнее оригинала (ревью ц.26)
    let active_wins = is_active && (is_light || !has_color);
    if active_wins {
        let (a1, a2, ab) = if is_light {
            (0.42, 0.26, 0.60)
        } else {
            (0.26, 0.14, 0.45)
        };
        tab = tab
            .bg(linear_gradient(
                90.,
                linear_color_stop(tint(tab_color, a1), 0.0),
                linear_color_stop(tint(tab_color, a2), 1.0),
            ))
            .border_color(tint(tab_color, ab))
            .text_color(rgba(p.text_primary));
    } else if has_color {
        // сюда попадает И активная цветная в тёмной теме — так у оригинала
        // .tinted 15%→8% (light 26→16); hover 22%→12% (градиент цвета)
        let (t1, t2) = if is_light { (0.26, 0.16) } else { (0.15, 0.08) };
        tab = tab
            .bg(linear_gradient(
                90.,
                linear_color_stop(tint(tab_color, t1), 0.0),
                linear_color_stop(tint(tab_color, t2), 1.0),
            ))
            .hover({
                let hover_fg = rgba(p.text_primary);
                move |st| {
                    st.bg(linear_gradient(
                        90.,
                        linear_color_stop(tint(tab_color, 0.22), 0.0),
                        linear_color_stop(tint(tab_color, 0.12), 1.0),
                    ))
                    .text_color(hover_fg)
                }
            });
    } else {
        // Обычный неактивный: hover = bg-surface + text-primary
        let hover_bg = rgba(p.bg_surface);
        let hover_fg = rgba(p.text_primary);
        tab = tab.hover(move |st| st.bg(hover_bg).text_color(hover_fg));
    }

    tab
}
