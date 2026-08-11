//! Инфо-элемент статус-бара: глиф, метка, тон, тултип.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use crate::ui::icon::codicon;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

pub(crate) const CIRCLE_FILLED: &str = "\u{ea71}";
pub(crate) const WARNING: &str = "\u{ea6c}";
pub(crate) const CIRCLE_SLASH: &str = "\u{eabd}";
pub(crate) const SYMBOL_KEYWORD: &str = "\u{eb62}";
/// Инфо-элемент статус-бара (glyph опц + label + tone-цвет + tooltip).
pub(crate) fn item(
    glyph: Option<&'static str>,
    label: impl Into<SharedString>,
    tone: gpui::Rgba,
    tip: &'static str,
    p: &Palette,
    // Клик по счётчику. У оригинала встроенные элементы информационные
    // (`StatusBar.tsx:147-159`, `tabIndex -1`, без onClick) — навигация тут
    // ДОБАВЛЕНА по просьбе пользователя, это осознанное отступление
    on_click: Option<Box<dyn Fn()>>,
) -> AnyElement {
    let hover_bg = tint(rgba(p.bg_surface), 0.6);
    // `.item:hover { color: text-primary }` (0,2,0) перебивает `.ok`/`.warn`:
    // на ховере И глиф, И подпись уходят в `--text-primary`. Собственный
    // `.hover().text_color()` до дочерних элементов не доходит (замерено
    // ревью ц.20) — красим через группу.
    let group = SharedString::from(format!("sb-{tip}"));
    let hover_fg = rgba(p.text_primary);
    let mut el = div()
        .id(SharedString::from(tip))
        .group(group.clone())
        .relative()
        // Поэлементный кроп: регион у ОДНОГО, детерминированно выбранного
        // элемента — иначе реестр хранил бы последний отрисованный
        .when(tip == "Active extensions", |d| {
            d.child(crate::probe::registry::probe_area("status-item"))
        })
        .flex()
        // Счётчик шириной со свою подпись: сжимать его нельзя, иначе
        // «0 active» переносится по словам и уезжает за нижний край.
        .flex_shrink_0()
        .items_center()
        .gap(px(4.0))
        .px(px(m::SPACE_2))
        .rounded(px(m::RADIUS_XS))
        .text_size(px(m::FS_XS))
        .text_color(tone)
        .hover(move |s| s.bg(hover_bg).text_color(rgba(p.text_primary)))
        .tooltip(crate::ui::tooltip::tooltip(tip));
    if let Some(g) = glyph {
        el = el.child(
            codicon(g, 12.0)
                .text_color(tone)
                .group_hover(group.clone(), move |st| st.text_color(hover_fg)),
        );
    }
    let mut el = el.child(
        div()
            .child(label.into())
            .group_hover(group, move |st| st.text_color(hover_fg)),
    );
    let clickable = on_click.is_some();
    if let Some(cb) = on_click {
        el = el
            .cursor_pointer()
            .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
                cb();
            });
    }
    // `tabIndex={-1}` (`StatusBar.tsx:154`, комментарий оригинала прямо
    // говорит: «keeps them out of the tab order — no no-op activation»).
    // Мы чеканили таб-стоп ВСЕМ, включая «N failed», «N off», UTF-8 и EOL —
    // и сами себе противоречили: `contrib()` некликабельный элемент из
    // таб-порядка исключает (ревью ц.25)
    if !clickable {
        return el.into_any_element();
    }
    crate::ui::focus_ring::focusable(
        el,
        &format!("sbi:{tip}"),
        m::RADIUS_XS,
        rgba(p.accent_primary),
    )
    .into_any_element()
}
