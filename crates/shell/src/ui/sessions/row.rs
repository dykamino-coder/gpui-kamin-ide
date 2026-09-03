//! Строка сессии в сайдбаре.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::tint;
use crate::colors::{parse_hex, rgba};
use crate::host_link::{HoverPillSource, ShellEvent};
use crate::ui::sessions::actions::label_deduction;
use crate::ui::sessions::actions::menu_data;
use crate::ui::sessions::pill::{anchor_probe, pin_btn};
use gpui::prelude::*;
use gpui::{AnyElement, Entity, SharedString, div, linear_color_stop, linear_gradient, px};
use gpui_component::input::InputState;
use kamin_metrics as m;
use kamin_model::Session;
use kamin_theme::Palette;
use smol::channel::Sender;

pub(crate) fn session_row(
    s: &Session,
    // Ширина сайдбара прошлого кадра — бюджет усечения метки
    sidebar_w: f32,
    is_active: bool,
    rename: Option<&Entity<InputState>>,
    hovered: bool,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    let tab_color = s
        .color
        .as_deref()
        // `resolveSessionColor`: светлая тема берёт насыщенный вариант
        .map(|hex| {
            parse_hex(
                crate::ui::context_menu::resolve_session_color(hex),
                rgba(p.accent_primary),
            )
        })
        .unwrap_or(rgba(p.accent_primary));
    let has_color = s.color.is_some();
    let hover_bg = tint(rgba(p.bg_surface), 0.55);

    let hover_group: SharedString = format!("srow-{}", s.id).into();
    let mut row = div()
        .id(SharedString::from(s.id.clone()))
        .group(hover_group.clone())
        .flex()
        .items_center()
        .gap(px(m::SPACE_2))
        .w_full()
        // Без min-w 0 строка не сжимается ниже интринсика содержимого
        .min_w(px(0.))
        .h(px(24.0))
        .pl(px(16.0))
        .pr(px(8.0))
        .border_1()
        .border_color(gpui::transparent_black())
        .rounded(px(m::RADIUS_XS))
        .text_size(px(m::FS_SM))
        .text_color(rgba(p.text_secondary))
        .cursor_pointer()
        .overflow_hidden();

    // ВАЖНО: у gpui ровно ОДИН `.hover()` на элемент — второй вызов
    // перезаписывает первый (и ловится `debug_assert` в dev). Поэтому все
    // ховер-эффекты строки собираются в один стиль.
    let open = s.open;
    let hf = rgba(p.text_primary);
    // Светлая тема: мягкие тинты вымываются на почти белой подложке, поэтому
    // у оригинала свои, более насыщенные проценты (`SessionItem.module.css:
    // 39-51,122`) — у нас стояли dark-значения для обеих тем (ревью ц.13)
    let light = kamin_theme::current_is_light();
    let (t_a, t_b) = if light { (0.26, 0.16) } else { (0.24, 0.13) };
    let (th_a, th_b) = if light { (0.34, 0.22) } else { (0.30, 0.17) };
    let (act_a, act_b, act_bd) = if light {
        (0.42, 0.26, 0.60)
    } else {
        (0.26, 0.14, 0.45)
    };
    // `.inactive { opacity: .6 }`, светлая — 0.8
    let dim = if light { 0.8 } else { 0.6 };
    if is_active {
        row = row
            .bg(linear_gradient(
                90.,
                linear_color_stop(tint(tab_color, act_a), 0.0),
                linear_color_stop(tint(tab_color, act_b), 1.0),
            ))
            .border_color(tint(tab_color, act_bd))
            .text_color(rgba(p.text_primary));
        // `.active:hover` держит тот же градиент; для закрытой строки
        // добавляем только возврат прозрачности
        if !open {
            row = row.opacity(dim).hover(|st| st.opacity(1.0));
        }
    } else if has_color {
        row = row.bg(linear_gradient(
            90.,
            linear_color_stop(tint(tab_color, t_a), 0.0),
            linear_color_stop(tint(tab_color, t_b), 1.0),
        ));
        if !open {
            row = row.opacity(dim);
        }
        // `.tinted:hover` = 30/17 (оригинал SessionItem.module.css)
        row = row.hover(move |st| {
            // `.tinted:hover` переопределяет только ФОН, цвет остаётся от
            // `.row:hover` = text-primary (css:23 против :28) — ревью ц.23
            let st = st
                .bg(linear_gradient(
                    90.,
                    linear_color_stop(tint(tab_color, th_a), 0.0),
                    linear_color_stop(tint(tab_color, th_b), 1.0),
                ))
                .text_color(hf);
            if open { st } else { st.opacity(1.0) }
        });
    } else {
        if !open {
            row = row.opacity(dim);
        }
        row = row.hover(move |st| {
            let st = st.bg(hover_bg).text_color(hf);
            if open { st } else { st.opacity(1.0) }
        });
    }

    let dot = crate::ui::sessions::status_dot::status_dot(s, is_active, tab_color, p);

    // Inline-переименование: вместо имени — фокусный Input (Enter/Esc).
    if let Some(input) = rename {
        return crate::ui::sessions::rename_row::rename_row(
            input, row, light, is_active, has_color, dot, tx, p,
        );
    }

    let id = s.id.clone();
    // `onClick` оригинала = mouse-UP: нажатие с уводом курсора сессию не
    // переключает; двойной клик открывает переименование
    // (`SessionItem.tsx:95-96`, ревью ц.20)
    let row = row
        .on_mouse_up(gpui::MouseButton::Left, {
            let tx = tx.clone();
            move |ev: &gpui::MouseUpEvent, _, _| {
                if ev.click_count >= 2 {
                    let _ = tx.try_send(ShellEvent::BeginRename(id.clone()));
                    return;
                }
                let _ = tx.try_send(ShellEvent::ActivateSession(id.clone()));
            }
        })
        .on_mouse_down(gpui::MouseButton::Right, {
            let tx = tx.clone();
            let data = menu_data(s);
            move |e: &gpui::MouseDownEvent, _, cx| {
                cx.stop_propagation();
                let _ = tx.try_send(ShellEvent::OpenSessionMenu(
                    data.clone(),
                    f32::from(e.position.x),
                    f32::from(e.position.y),
                ));
            }
        })
        // Ховер строки → показать/увести hover-поповер (по id).
        .on_hover({
            let tx = tx.clone();
            let id = s.id.clone();
            move |h: &bool, _, _| {
                let _ = tx.try_send(ShellEvent::HoverPill {
                    id: id.clone(),
                    source: HoverPillSource::Anchor,
                    hovered: *h,
                });
            }
        })
        .child(dot)
        .child(
            // `text-overflow: ellipsis`: gpui рисует «…» только когда ширина
            // текстового элемента ОПРЕДЕЛЕНА. У `flex_1` она резолвится позже,
            // поэтому текст кладём в дочерний `w_full`-бокс (ревью ц.16)
            div().flex_1().min_w(px(0.)).overflow_hidden().child(
                div()
                    .w_full()
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    // Многоточие дописывает `text_fit` — движок его не рисует
                    // (ревью ц.20/21). Бюджет: ширина сайдбара минус инсеты
                    // строки (pl 16 + pr 8), точка 4, гэп 8 и колонка времени 32
                    .child(crate::ui::text_fit::fit_approx(
                        &s.name,
                        sidebar_w - label_deduction(s),
                        m::FS_SM,
                    )),
            ),
        )
        // `.time` с `data-tooltip={absoluteTime(...)}` (`SessionItem.tsx:110`)
        .child(
            div()
                .id(SharedString::from(format!("srow-time:{}", s.id)))
                .tooltip(crate::ui::tooltip::tooltip(
                    crate::ui::time_fmt::absolute_ms(s.last_opened),
                ))
                .flex_shrink_0()
                .text_size(px(m::FS_XS))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(rgba(p.text_muted))
                .opacity(0.7)
                .child(crate::ui::time_fmt::relative_ms(s.last_opened)),
        )
        // pin — инлайн (fa-thumbtack), виден по ховеру или всегда если pinned.
        .child(pin_btn(s, hover_group, tab_color, p))
        // Hover-пилюля рисуется в OVERLAY-окне (поверх вебвью) — здесь только
        // якорь позиции hovered-строки.
        .when(hovered, |r| r.child(anchor_probe(s.id.clone())));
    // Радиус кольца = радиусу САМОЙ строки (`.item { border-radius: 4 }`),
    // стояло 8 (ревью ц.35)
    crate::ui::focus_ring::focusable(
        row,
        &format!("srow:{}", s.id),
        m::RADIUS_XS,
        rgba(p.accent_primary),
    )
    .into_any_element()
}
