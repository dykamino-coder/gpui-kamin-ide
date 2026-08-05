//! Fly-out пилюля строки: якорь, кнопки, обёртка.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::colors::tint;
use crate::host_link::{self, HoverPillSource, ShellEvent};
use crate::ui::icon::{FA_FAMILY, codicon, fa};
use crate::ui::sessions::glyphs::FA_THUMBTACK;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_model::Session;
use kamin_theme::Palette;
use serde_json::json;
use smol::channel::Sender;

/// Якорь hovered-строки (лог. px) — overlay рисует пилюлю по нему.
pub fn pill_anchor() -> &'static std::sync::Mutex<Option<[f32; 4]>> {
    static S: std::sync::OnceLock<std::sync::Mutex<Option<[f32; 4]>>> = std::sync::OnceLock::new();
    S.get_or_init(Default::default)
}
pub(crate) fn anchor_probe() -> impl gpui::IntoElement {
    gpui::canvas(
        |bounds, _, _| {
            *pill_anchor().lock().unwrap() = Some([
                f32::from(bounds.origin.x),
                f32::from(bounds.origin.y),
                f32::from(bounds.size.width),
                f32::from(bounds.size.height),
            ]);
        },
        |_, _, _, _| {},
    )
    // Инсеты вместо `size_full` — иначе канвас участвует в раскладке строки
    // (см. `probe_registry::probe_area`)
    .absolute()
    .top_0()
    .left_0()
    .right_0()
    .bottom_0()
}
/// Инлайн-пин строки (fa-thumbtack 10px): по ховеру или всегда если pinned.
pub(crate) fn pin_btn(
    s: &Session,
    group: SharedString,
    tab_color: gpui::Rgba,
    p: &Palette,
) -> AnyElement {
    let pinned = s.pinned;
    let mut btn = div()
        .id(SharedString::from(format!("pin-{}", s.id)))
        .flex_shrink_0()
        .w(px(20.0))
        .h(px(20.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(m::RADIUS_XS))
        .cursor_pointer()
        .text_color(if pinned {
            tab_color
        } else {
            rgba(p.text_muted)
        })
        // `.pin:hover { color: var(--tab-color) }` — БЕЗ фона
        .hover(move |s| s.text_color(tab_color).opacity(1.0))
        .tooltip(crate::ui::tooltip::tooltip(if pinned {
            "Unpin from top bar"
        } else {
            "Pin to top bar"
        }))
        .on_mouse_down(gpui::MouseButton::Left, {
            let id = s.id.clone();
            move |_, _, cx| {
                cx.stop_propagation();
                let id = id.clone();
                std::thread::spawn(move || {
                    if let Some(c) = host_link::client() {
                        let _ =
                            c.request("kamin:sessions:setPinned", vec![json!(id), json!(!pinned)]);
                    }
                });
            }
        })
        .child(
            fa(FA_THUMBTACK, 10.0)
                .font_family(FA_FAMILY)
                .w(px(14.0))
                .h(px(14.0)),
        );
    if !pinned {
        // `.action { display: none }` — скрытая кнопка НЕ занимает места
        // (`invisible()` в gpui оставляет бокс, и лейбл был на 28px уже:
        // 20 бокса + 8 гэпа — ревью ц.13). Схлопываем ширину и гасим гэп
        // отрицательным отступом, на ховере строки — возвращаем.
        // `.row:hover .action { opacity: 0.7 }`, свой ховер → 1.
        btn = btn
            .w(px(0.))
            .ml(px(-m::SPACE_2))
            .overflow_hidden()
            .invisible()
            .group_hover(group, |s| s.visible().opacity(0.7).w(px(20.0)).ml(px(0.)));
    }
    btn.into_any_element()
}
/// Кнопка внутри hover-пилюли — 24×24, hover-подсветка.
#[allow(clippy::too_many_arguments)]
pub(crate) fn pill_btn(
    id: String,
    glyph: &'static str,
    tip: &'static str,
    hover_fg: Option<gpui::Rgba>,
    red_bg: bool,
    // Кегль глифа: у сессии 13 (`SessionItem.module.css:168`), у пилюли
    // проекта 14 (`ProjectGroup.module.css:96`) — раньше обе рисовались 13
    glyph_px: f32,
    p: &Palette,
    on_click: impl Fn() + 'static,
) -> AnyElement {
    // Оригинал: `.popAction:hover` = bg text-primary 12% + color text-primary,
    // модификаторы меняют ТОЛЬКО цвет (rename/add → accent, disconnect → blue,
    // delete → red). У ProjectGroup `.delete:hover` ещё и красный фон 15%.
    let hover_bg = if red_bg {
        tint(rgba(p.accent_red), 0.15)
    } else {
        tint(rgba(p.text_primary), 0.12)
    };
    let hover_fg = hover_fg.unwrap_or(rgba(p.text_primary));
    let fid = format!("pill:{id}");
    let b = div()
        .id(SharedString::from(id))
        .w(px(24.0))
        .h(px(24.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(m::RADIUS_XS))
        .cursor_pointer()
        .text_color(rgba(p.text_secondary))
        .hover(move |s| s.bg(hover_bg).text_color(hover_fg))
        .tooltip(crate::ui::tooltip::tooltip(tip))
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            on_click();
        })
        .child(codicon(glyph, glyph_px));
    crate::ui::focus_ring::focusable(b, &fid, m::RADIUS_XS, rgba(p.accent_primary))
        .into_any_element()
}
/// Визуальная обёртка; составной hitbox задаёт `overlay_pill`.
pub(crate) fn pill_wrap(id: String, p: &Palette) -> gpui::Stateful<gpui::Div> {
    div()
        .id(SharedString::from(id))
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(px(2.0))
        .p(px(3.0))
        // скругление как у окружающих карточек (glint RADIUS_LG)
        .rounded(px(m::RADIUS_MD))
        .bg(rgba(p.bg_surface))
        .border_1()
        .border_color(tint(rgba(p.text_primary), 0.06))
        .shadow(vec![gpui::BoxShadow {
            color: gpui::Rgba {
                r: 0.,
                g: 0.,
                b: 0.,
                a: 0.35,
            }
            .into(),
            offset: gpui::point(px(0.), px(4.)),
            blur_radius: px(16.),
            spread_radius: px(0.),
        }])
}
/// Пилюля в overlay: absolute на якоре строки (справа от сайдбара).
pub fn overlay_pill(
    inner: AnyElement,
    anchor: [f32; 4],
    viewport: (f32, f32),
    pill_w: f32,
    hover_id: &str,
    tx: &Sender<ShellEvent>,
) -> AnyElement {
    const GUTTER: f32 = 8.0;
    const PILL_H: f32 = 32.0;
    const OFFSET: f32 = 4.0;
    // Как `.actionsPop::before`: перекрывает offset и 6px строки.
    const HOVER_BRIDGE: f32 = 10.0;
    let (vw, vh) = viewport;
    // Справа: a.right + 4; при нехватке места — flip влево. По вертикали
    // панель центрируется на якоре и клампится в viewport.
    let mut panel_left = anchor[0] + anchor[2] + OFFSET;
    let mut on_right = true;
    if panel_left + pill_w > vw - GUTTER {
        on_right = false;
        let flipped = anchor[0] - OFFSET - pill_w;
        panel_left = if flipped >= GUTTER {
            flipped
        } else {
            (vw - pill_w - GUTTER).max(GUTTER)
        };
    }
    let top = (anchor[1] + anchor[3] / 2.0 - PILL_H / 2.0)
        .min(vh - PILL_H - GUTTER)
        .max(GUTTER);
    // При flip мост справа; inner остаётся на прежнем panel_left.
    let hover_left = if on_right {
        panel_left - HOVER_BRIDGE
    } else {
        panel_left
    };
    let tx = tx.clone();
    let hover_id = hover_id.to_string();
    let wrapper = div()
        .id(SharedString::from(format!("pill-hover-{hover_id}")))
        .absolute()
        .left(px(hover_left.round()))
        .top(px(top.round()))
        .w(px(pill_w + HOVER_BRIDGE))
        .h(px(PILL_H))
        .flex()
        .items_center()
        .occlude()
        .on_hover(move |h: &bool, _, _| {
            let _ = tx.try_send(ShellEvent::HoverPill {
                id: hover_id.clone(),
                source: HoverPillSource::Panel,
                hovered: *h,
            });
        })
        // Visible region не включает мост: fallback без alpha остаётся чистым.
        .child(crate::overlay::input_area())
        .child(
            div()
                .relative()
                .w(px(pill_w))
                .h(px(PILL_H))
                .child(crate::overlay::hit_area())
                .child(inner),
        );
    if on_right {
        wrapper.justify_end().into_any_element()
    } else {
        wrapper.justify_start().into_any_element()
    }
}
