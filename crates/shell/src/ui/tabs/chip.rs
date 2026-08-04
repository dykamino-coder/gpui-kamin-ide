//! Чип сессии в титлбаре и его кнопка-действие.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::{parse_hex, rgba};
use crate::host_link::ShellEvent;
use crate::ui::session_tabs::Tx;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_model::Session;
use kamin_theme::Palette;

// Компонент дизайн-системы: аргументы — его пропсы.
#[allow(clippy::too_many_arguments)]
pub(crate) fn chip(
    s: &Session,
    first: bool,
    window: &mut gpui::Window,
    is_active: bool,
    dragging: bool,
    switching: bool,
    chip_w: f32,
    label_max_w: f32,
    tx: &Tx,
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

    // Light: сильнее тинты (tinted 26/16, active 42/26 + border 60) — CSS
    let is_light = kamin_theme::current_is_light();
    let mut tab = crate::ui::tabs::chip_style::chip_style(
        s, is_active, has_color, is_light, tab_color, first, chip_w, p,
    );
    let dot_color = if is_active {
        tab_color
    } else {
        rgba(p.text_muted)
    };
    // Sleeping (pinned+closed) — призрак: opacity 0.55 + label text-muted
    if s.pinned && !s.open {
        tab = tab.opacity(0.55).text_color(rgba(p.text_muted));
    }
    // .dndDragging: перетаскиваемый чип .4
    if dragging {
        tab = tab.opacity(0.4);
    }
    let id = s.id.clone();
    let group: SharedString = format!("chip-{}", s.id).into();
    // Leading (SessionTab.module.css .leading): точка 4px по умолчанию,
    // на ховере чипа слот меняется на КНОПКУ-пин (клик = toggle pin);
    // у pinned пин виден всегда и красится в tab-color. Спиннер при switching.
    let leading: AnyElement = crate::ui::tabs::chip_leading::leading(
        s, p, tab_color, dot_color, is_active, switching, &group, tx,
    );
    let tab = tab
        .group(group.clone())
        // ЕДИНСТВЕННЫЙ .tooltip на чип: второй вызов = паника gpui
        // (debug_assert «calling tooltip more than once»)
        .tooltip(crate::ui::tooltip::tooltip(if s.pinned && !s.open {
            format!("{} (sleeping — click to reactivate)", s.name)
        } else if switching {
            format!("{} (loading conversation…)", s.name)
        } else {
            s.name.clone()
        }))
        // Нажатие = кандидат drag-reorder; активация на mouse-up без движения.
        // stop_propagation ОБЯЗАТЕЛЕН: иначе титлбар начнёт start_window_move
        // (модальный OS-цикл) и съест mouse-move/up драга.
        .on_mouse_down(gpui::MouseButton::Left, {
            let tx = tx.clone();
            let id = id.clone();
            move |e: &gpui::MouseDownEvent, _, cx| {
                cx.stop_propagation();
                if e.click_count >= 2 {
                    // Dblclick = переименование сессии (как оригинал)
                    let _ = tx.try_send(ShellEvent::BeginRename(id.clone()));
                    return;
                }
                let _ = tx.try_send(ShellEvent::ChipPress(
                    id.clone(),
                    f32::from(e.position.x),
                    f32::from(e.position.y),
                ));
            }
        })
        // Зажатая ЛКМ над чипом → цель вставки reorder
        .on_mouse_move({
            let tx = tx.clone();
            let id = id.clone();
            move |e: &gpui::MouseMoveEvent, _, _| {
                if e.pressed_button == Some(gpui::MouseButton::Left) {
                    let _ = tx.try_send(ShellEvent::ChipDragOver(id.clone()));
                }
            }
        })
        // Отпускание НА чипе: occlude() чипа не пропускает up до root —
        // коммитим отсюда (root-обработчик остаётся для up вне чипов)
        .on_mouse_up(gpui::MouseButton::Left, {
            let tx = tx.clone();
            move |_, _, _| {
                let _ = tx.try_send(ShellEvent::ChipRelease);
            }
        })
        // RMB → то же контекст-меню, что у строки сайдбара
        .on_mouse_down(gpui::MouseButton::Right, {
            let tx = tx.clone();
            let data = crate::ui::sessions_list::menu_data(s);
            move |e: &gpui::MouseDownEvent, _, cx| {
                cx.stop_propagation();
                let _ = tx.try_send(ShellEvent::OpenSessionMenu(
                    data.clone(),
                    f32::from(e.position.x),
                    f32::from(e.position.y),
                ));
            }
        })
        .child(
            div()
                .w(px(16.0))
                .h(px(16.0))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .child(leading),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .overflow_hidden()
                .font_weight(gpui::FontWeight::MEDIUM)
                // Внутренний w_full: gpui рисует «…» только когда у ТЕКСТА
                // определённая ширина; на flex_1 без обёртки текст просто
                // обрезался (скрин-сверка с оригиналом)
                // max_w в px: gpui-truncate требует ОПРЕДЕЛЁННУЮ ширину
                // текста (label ≈ basis 180 − иконки/паддинги/гапы)
                .child(div().truncate().child(crate::ui::text_fit::fit(
                    &s.name,
                    label_max_w,
                    12.0,
                    window,
                ))),
        )
        .when(s.open, |t| {
            t.child(crate::ui::tabs::chip_action::chip_action(
                format!("tabdc-{}", s.id),
                crate::ui::tabs::chip_action::DISCONNECT_GLYPH,
                "Disconnect (free from memory)",
                group,
                p,
                {
                    let id = s.id.clone();
                    let tx = tx.clone();
                    move || {
                        let _ = tx.try_send(ShellEvent::LocalSessionClosed(id.clone()));
                        let id = id.clone();
                        std::thread::spawn(move || {
                            if let Some(c) = crate::host_link::client() {
                                let _ = c.request(
                                    "kamin:sessions:deactivate",
                                    vec![serde_json::json!(id)],
                                );
                            }
                        });
                    }
                },
                // Активный чип: disconnect ВСЕГДА виден (оригинал)
                is_active,
            ))
        });
    #[allow(clippy::let_and_return)]
    crate::ui::focus_ring::focusable(
        tab,
        &format!("chip:{}", s.id),
        m::RADIUS_MD,
        rgba(p.accent_primary),
    )
    .into_any_element()
}
/// flex-basis чипа (SessionTab.module.css: flex 0 1 180px)
pub(crate) const CHIP_W: f32 = 180.0;
/// min-width чипа — до этого сжимаются, дальше overflow-кнопка
pub(crate) const CHIP_MIN_W: f32 = 44.0;
/// Нетекстовая часть чипа: pl10 + pr6 + leading16 + close18 + 2 гапа по 6
/// плюс 2 рамки. У СПЯЩЕГО чипа кнопки disconnect нет в DOM вовсе
/// (`SessionTab.tsx:53`) — там на 24 px больше под метку (ревью ц.19).
pub(crate) const CHIP_CHROME_W: f32 = 64.0;
pub(crate) const CHIP_CHROME_W_SLEEPING: f32 = CHIP_CHROME_W - 24.0;
/// Левого отступа у `.strip` НЕТ: воздух слева даёт `.leftCluster`, пиннутый
/// к ширине сайдбара (досье 02). Пока обёртки не было, её подменяла константа
/// `STRIP_PL = 48` — с обёрткой отступ удвоился бы (ревью ц.35).
pub(crate) const CHIP_GAP: f32 = 3.0; // ml2 + mr1
