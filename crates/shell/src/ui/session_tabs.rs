//! Табы открытых сессий в титлбаре (SessionTab.module.css 1:1):
//! чип h28 radius 12, padding 0 6 0 10, gap 6, flex 0 1 180 (min 44 / max 240),
//! точка 4px; active = градиент 90° tab-color 26%→14% + бордер 45%;
//! tinted (цветной неактивный) 15%→8%. Пины/dnd/close — след. итерация.

use crate::ui::tabs::chip::CHIP_CHROME_W;
use crate::ui::tabs::chip::CHIP_CHROME_W_SLEEPING;
use crate::ui::tabs::chip::CHIP_GAP;
use crate::ui::tabs::chip::CHIP_MIN_W;
use crate::ui::tabs::chip::CHIP_W;
use crate::ui::tabs::chip::chip;
use crate::ui::tabs::overflow::overflow_button;
pub use crate::ui::tabs::overflow::{overflow_hidden_ids, tabs_overflow_menu};
use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_model::Session;
use kamin_theme::Palette;

use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;

pub(crate) type Tx = smol::channel::Sender<ShellEvent>;

/// Чипы в порядке отображения: пользовательский order (drag-reorder)
/// поверх базовой сортировки по created_at; неизвестные id — в конец.
/// Порядок оригинала (`signals/sessions.ts::openSessions`): sleeping
/// (закреплённые, но НЕ открытые) — СЛЕВА, активные — СПРАВА у «+»;
/// внутри групп стабильный порядок (plan/99 п.39).
///
/// База — ВРЕМЯ СОЗДАНИЯ, как в оригинале («stable creation order»).
/// Стояло `last_opened` — выбор сессии обновлял её время, и табы менялись
/// местами при каждом переключении (жалоба со скриншотом). Группировка по
/// `open` стабильна: флаг меняется только явным connect/disconnect.
pub fn ordered_chips<'a>(sessions: &'a [Session], order: &[String]) -> Vec<&'a Session> {
    let mut open: Vec<&Session> = sessions.iter().filter(|s| s.open || s.pinned).collect();
    open.sort_by(|a, b| a.created_at.total_cmp(&b.created_at));
    let pos = |s: &Session| order.iter().position(|id| *id == s.id);
    open.sort_by(|a, b| match (pos(a), pos(b)) {
        (Some(x), Some(y)) => x.cmp(&y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal, // stable → created_at
    });
    open.sort_by_key(|s| s.open); // sleeping (false) слева, активные справа
    open
}

// Компонент дизайн-системы: аргументы — его пропсы.
#[allow(clippy::too_many_arguments)]
/// Слот табов: ВСЕ чипы одного размера (180px); не влезшие уходят под
/// кнопку «N ⌄» с поповером. Активная сессия всегда видима.
pub fn session_tabs(
    sessions: &[Session],
    active_id: Option<&str>,
    order: &[String],
    drag_over: Option<&str>,
    dragging: Option<&str>,
    switching: Option<&str>,
    available_w: f32,
    overflow_open: bool,
    on_toggle_overflow: impl Fn(f32, f32) + 'static,
    tx: &Tx,
    p: &Palette,
    window: &mut gpui::Window,
) -> AnyElement {
    // Открытые + sleeping (pinned+closed — призрак, клик реактивирует)
    let open = ordered_chips(sessions, order);

    // Сколько чипов влезает (резерв 36px под overflow-кнопку при переполнении)
    let total = open.len();
    // Влезаемость считаем по MIN-ширине: до неё чипы сжимает flex
    let usable_w = available_w.max(CHIP_MIN_W);
    let fit_all = (total as f32) * (CHIP_MIN_W + CHIP_GAP) <= usable_w;
    let fit = if fit_all {
        total
    } else {
        (((usable_w - 36.0) / (CHIP_MIN_W + CHIP_GAP)).floor() as usize).clamp(1, total)
    };
    // Активная всегда на экране: если она за границей — подтянуть в конец видимых
    let mut visible: Vec<&Session> = open.iter().take(fit).copied().collect();
    let mut hidden: Vec<&Session> = open.iter().skip(fit).copied().collect();
    if let Some(aid) = active_id
        && !visible.iter().any(|s| s.id == aid)
        && let Some(pos) = hidden.iter().position(|s| s.id == aid)
    {
        let active = hidden.remove(pos);
        if let Some(last) = visible.pop() {
            hidden.insert(0, last);
        }
        visible.push(active);
    }

    let mut row = div()
        .relative()
        // Досье 18 — сам СТРИП; `tabs-slot` это слот титлбара (досье 04),
        // и общий регион давал им один кроп (ревью ц.26)
        .child(crate::probe::registry::probe_area("session-strip"))
        .flex()
        .items_center()
        .min_w(px(0.))
        // `.strip { height: 100% }` (`SessionTabs.module.css:1-10`) в слоте
        // 42 px: у чипа 28 остаётся по 7 px сверху и снизу, и кольцо фокуса
        // (offset 2 + 2) видно целиком. Без этого ряд был высотой в чип и
        // `overflow_hidden` срезал кольцо (ревью ц.25)
        .h_full()
        .overflow_hidden()
        // Своих боковых отступов у `.strip` нет: слева воздух даёт
        // `.leftCluster`, справа зазор до «+» — его собственный mx(6)
        ;
    // Фактическая ширина чипа: равный flex-shrink от basis 180, кламп
    // [44..180] — gpui-truncate не умеет резать текст по итоговой ширине,
    // поэтому усечение считаем сами (см. text_fit)
    let vis_n = visible.len().max(1) as f32;
    let row_w = (available_w
        - 4.0 // доп. ml первого чипа (6 против 2)
        - if fit_all { 0.0 } else { 36.0 })
    .max(0.0);
    let chip_w = (row_w / vis_n - CHIP_GAP).clamp(CHIP_MIN_W, CHIP_W).floor();
    let label_max_w = (chip_w - CHIP_CHROME_W).max(12.0);
    let label_max_w_sleeping = (chip_w - CHIP_CHROME_W_SLEEPING).max(12.0);
    for (i, s) in visible.into_iter().enumerate() {
        // .dropBar: отдельная полоса 2×22 r1 accent + glow ПЕРЕД целью
        // вставки (ревью ц.1: раньше border_l красил сам чип)
        if drag_over == Some(s.id.as_str()) {
            row = row.child(
                div()
                    .w(px(2.0))
                    .h(px(22.0))
                    .mx(px(1.0))
                    .flex_shrink_0()
                    .rounded(px(1.0))
                    .bg(rgba(p.accent_primary))
                    .shadow(vec![gpui::BoxShadow {
                        color: tint(rgba(p.accent_primary), 0.6).into(),
                        offset: gpui::point(px(0.), px(0.)),
                        blur_radius: px(4.),
                        spread_radius: px(0.),
                    }]),
            );
        }
        row = row.child(chip(
            s,
            i == 0,
            window,
            active_id == Some(s.id.as_str()),
            dragging == Some(s.id.as_str()),
            switching == Some(s.id.as_str()),
            chip_w,
            // У СПЯЩЕГО чипа кнопки disconnect нет в DOM — метке достаётся
            // на 24 px больше (`SessionTab.tsx:53`, ревью ц.19)
            if s.open {
                label_max_w
            } else {
                label_max_w_sleeping
            },
            tx,
            p,
        ));
    }
    // Публикуем скрытых для overlay-меню (рисуется в overlay-окне —
    // main-дропдаун уходил ПОД wv2-чайлд чата)
    *overflow_hidden_ids().lock().unwrap() = hidden.iter().map(|s| s.id.clone()).collect();
    if !hidden.is_empty() {
        row = row.child(overflow_button(
            &hidden,
            overflow_open,
            on_toggle_overflow,
            p,
        ));
    }
    row.into_any_element()
}
