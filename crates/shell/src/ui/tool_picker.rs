//! Пикер тулзов слота (ActivityPicker 1:1): поповер min-w 220, bg-surface,
//! заголовок «Tools», по строке на тул реестра (иконка + label + check у
//! pinned). Клик: pinned → unpin (toggle-off), иначе pin (+активация).
//! Открывается «...»-кнопкой стрипа и «Open Tool»-пилюлей плейсхолдера.
//! Рендер — overlay-слой; hit_area в корне; скрим-закрытие делает main.

pub use crate::ui::picker_parts::picker_anchor_id;

use crate::ui::picker_parts::{CHECK, PICKER_W, SINGLE, slot_label, tool_icon};
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

use crate::activity::{ActivityModel, BUILTIN_ACTIVITIES, PanelSlot};
use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;

pub(crate) const MARGIN: f32 = 8.0;

// Компонент дизайн-системы: аргументы — его пропсы.
#[allow(clippy::too_many_arguments)]
pub fn tool_picker(
    slot: PanelSlot,
    x: f32,
    y: f32,
    up: bool,
    model: &ActivityModel,
    tx: &Sender<ShellEvent>,
    viewport_w: f32,
    viewport_h: f32,
    window: &mut gpui::Window,
    p: &Palette,
) -> AnyElement {
    let pinned: Vec<String> = model.state(slot).pinned.clone();
    // builtin + contributed (view-контейнеры расширений) одним списком
    let mut all: Vec<(String, String, String)> = BUILTIN_ACTIVITIES
        .iter()
        .map(|a| (a.id.to_string(), a.label.to_string(), a.icon.to_string()))
        .collect();
    all.extend(
        crate::activity::dyn_tools_list()
            .into_iter()
            .map(|d| (d.id, d.label, d.icon)),
    );

    // Геометрия меню считается по СОДЕРЖИМОМУ (ревью ц.13 показало, что и
    // прежние константы 34/35.2, и «40» шапки взяты с потолка):
    //   шапка   = p 4×2 + рамка 1×2 + (py 4×2 + FS_XS·1.169) + gap 1 ≈ 31.9
    //   строка  = py 8×2 + max(глиф, текст FS_SM·1.169, галка 16)
    //             → 34 у svg-иконки (18) и 32 у codicon (16)
    //   ширина  = px 12×2 + глиф 18 + gap 8 + label [+ gap 8 + галка 16]
    //             + p 4×2 + рамка 2, но НЕ прибавляется к `min-w` 220:
    //             при `box-sizing: border-box` она уже внутри (ревью ц.13)
    const HEAD_H: f32 = 31.9;
    let lh = |fs: f32| fs * 1.169;
    let mut rows_h = 0.0_f32;
    let mut content_w = 0.0_f32;
    for (id, label, icon) in &all {
        let is_svg = crate::ui::activity_bar::phosphor_path(icon).is_some();
        let glyph: f32 = if is_svg { 18.0 } else { 16.0 };
        rows_h += m::SPACE_2 * 2.0 + glyph.max(lh(m::FS_SM)).max(16.0);
        // Правый край строки: галка, замок-метка одиночки или имя панели,
        // где одиночка сейчас живёт
        let tail = if pinned.iter().any(|x| x == id) {
            m::SPACE_2 + 16.0
        } else if crate::activity::is_singleton(id) {
            match model.slot_of(id) {
                Some(home) if home != slot => {
                    m::SPACE_2 + crate::ui::text_fit::measure(slot_label(home), m::FS_XS, window)
                }
                _ => m::SPACE_2 + 12.0,
            }
        } else {
            0.0
        };
        content_w = content_w.max(
            m::SPACE_3 * 2.0
                + glyph
                + m::SPACE_2
                + crate::ui::text_fit::measure(label, m::FS_SM, window)
                + tail,
        );
    }
    // `gap: 1` МЕЖДУ строками: в HEAD_H заложен только один зазор (шапка →
    // первая строка), остальные N−1 забыли — на двенадцати тулах кламп
    // промахивался на 11px (ревью ц.14)
    let est_h = HEAD_H + rows_h + (all.len().saturating_sub(1)) as f32;
    let menu_w = (content_w + m::SPACE_1 * 2.0 + 2.0)
        .max(PICKER_W)
        .min((viewport_w - 16.0).max(PICKER_W));
    // `clampToViewport` якорится РЕКТОМ триггера и центрирует меню по его
    // кросс-оси (`clamp-popup.ts:97-112`); точка клика давала смещение до
    // 108px (ревью ц.15). Флип — только если на выбранной стороне не хватает
    // места, а на противоположной хватает.
    let (px_x, px_y) = match crate::probe::registry::bounds_of(picker_anchor_id(slot)) {
        Some([ax, ay, aw, ah]) => {
            let above = ay - 6.0 - est_h;
            let below = ay + ah + 6.0;
            let top = if up {
                if above < MARGIN && viewport_h - (ay + ah) - 6.0 - est_h >= MARGIN {
                    below
                } else {
                    above
                }
            } else if below + est_h > viewport_h - MARGIN && above >= MARGIN {
                above
            } else {
                below
            };
            (
                (ax + aw / 2.0 - menu_w / 2.0)
                    .clamp(MARGIN, (viewport_w - menu_w - MARGIN).max(MARGIN)),
                top,
            )
        }
        // Фолбэк до первого замера триггера — прежняя точка клика
        None => (
            x.min(viewport_w - menu_w - MARGIN).max(MARGIN),
            if up {
                (y - est_h - 6.0).max(MARGIN)
            } else {
                y + 6.0
            }
            .min(viewport_h - est_h - MARGIN)
            .max(MARGIN),
        ),
    };

    let mut col = div()
        .id("tool-picker")
        .occlude()
        .absolute()
        .left(px(px_x))
        .top(px(px_y))
        // `.menu` — min-width 220 (ширина тянется под длинные названия),
        // gap 1; `.menuPortal` — max-height 100vh − 16 со своим скроллом
        .min_w(px(PICKER_W))
        // `.menuPortal { max-width: calc(100vw − 16px) }` — длинное имя
        // contributed-тула иначе уводит меню за правый край
        .max_w(px((viewport_w - 16.0).max(PICKER_W)))
        .max_h(px((viewport_h - 16.0).max(80.0)))
        // `.menuPortal { overflow-y: auto }` — длинный список скроллится
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap(px(1.0))
        .p(px(m::SPACE_1))
        .rounded(px(m::RADIUS_MD))
        .bg(rgba(p.bg_surface))
        .border_1()
        .border_color(tint(rgba(p.text_primary), 0.06))
        .shadow(crate::overlay::dropdown_shadow())
        .child(crate::overlay::hit_area())
        .child(
            div()
                .px(px(m::SPACE_3))
                .py(px(m::SPACE_1))
                .text_size(px(m::FS_XS))
                .letter_spacing(px(m::FS_XS * 0.04))
                .text_color(rgba(p.text_muted))
                .child("TOOLS"),
        );

    for (item_id, item_label, item_icon) in all {
        let is_pinned = pinned.contains(&item_id);
        // Тул-одиночка: живёт в одном слоте. Если он уже открыт в ДРУГОЙ
        // панели — строка неактивна, вместо галки написано, где он сейчас.
        let single = crate::activity::is_singleton(&item_id);
        let home = model.slot_of(&item_id);
        let locked = single && home.is_some_and(|h| h != slot);
        let hover_bg = tint(rgba(p.text_primary), 0.10);
        let tx = tx.clone();
        let item_id = item_id.clone();
        col = col.child(
            div()
                .id(SharedString::from(format!("tp-{item_id}")))
                .flex()
                .items_center()
                .gap(px(m::SPACE_2))
                .px(px(m::SPACE_3))
                .py(px(m::SPACE_2))
                .rounded(px(m::RADIUS_SM))
                .text_size(px(m::FS_SM))
                .text_color(rgba(p.text_primary))
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    // Одиночка из другой панели ПЕРЕНОСИТСЯ сюда (пропадает
                    // там) — раньше строка была заблокирована (запрос юзера).
                    let ev = if locked {
                        ShellEvent::MoveToolTo(home.unwrap(), item_id.clone(), slot)
                    } else if is_pinned {
                        ShellEvent::UnpinTool(slot, item_id.clone())
                    } else {
                        ShellEvent::PinTool(slot, item_id.clone())
                    };
                    let _ = tx.try_send(ev);
                })
                .child(tool_icon(&item_icon, p))
                .child(div().flex_1().child(item_label.clone()))
                // Пометка одиночки: замок 12 приглушённым — «экземпляр один»
                // Замок — только если тул НЕ закреплён здесь: у запиненной
                // одиночки иначе рисовались И замок, И галка (ревью ц.14)
                .when(single && !locked && !is_pinned, |row| {
                    row.child(crate::ui::icon::codicon(SINGLE, 12.0).text_color(rgba(p.text_muted)))
                })
                // Занят другой панелью — вместо галки её имя
                .when(locked, |row| {
                    row.child(
                        div()
                            .text_size(px(m::FS_XS))
                            .text_color(rgba(p.text_muted))
                            .child(SharedString::from(slot_label(home.unwrap()))),
                    )
                })
                .when(is_pinned, |row| {
                    row.child(
                        // `.codicon` без переопределений — 16px, цвет наследуется
                        crate::ui::icon::codicon(CHECK, 16.0),
                    )
                }),
        );
    }
    col.into_any_element()
}
