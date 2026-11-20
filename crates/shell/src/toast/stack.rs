//! Стопка тостов: слоты, раскладка, очередь и переполнение.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::toast::STATE;
use crate::toast::TICKER;
use crate::toast::open_window;
use crate::toast::win32::work_area;
use crate::toast::win32::{place_window, raw_hwnd};
use crate::toast::{HARD_MAX_VISIBLE, HEIGHT, MARGIN, STEP, WIDTH};
use gpui::prelude::*;
use gpui::{AnyWindowHandle, App};

/// Зазор между карточками стопки: прежний шаг минус прежняя высота.
const GAP: f32 = STEP - HEIGHT;

/// Верхние края окон стопки снизу вверх: высота у каждого своя — заголовок
/// переносится до трёх строк, и карточка растёт по содержимому.
pub(crate) fn stack_tops(bottom: f32, heights: &[f32]) -> Vec<f32> {
    let mut edge = bottom;
    heights
        .iter()
        .map(|h| {
            let top = edge - h;
            edge = top - GAP;
            top
        })
        .collect()
}

/// Влезет ли ещё одна карточка (пока базовой высоты) над уже открытыми.
pub(crate) fn has_room(heights: &[f32], work_height: f32) -> bool {
    if heights.len() >= HARD_MAX_VISIBLE {
        return false;
    }
    let used: f32 = heights.iter().map(|h| h + GAP).sum();
    heights.is_empty() || used + HEIGHT <= work_height - 2.0 * MARGIN
}

fn heights() -> Vec<f32> {
    STATE
        .lock()
        .map(|inner| inner.active.iter().map(|a| a.height).collect())
        .unwrap_or_default()
}

/// Место для ещё одного тоста по высоте рабочей области.
pub(crate) fn room_for(cx: &App, open: usize) -> bool {
    let (_, _, _, wh) = work_area(cx);
    let mut h = heights();
    h.extend(std::iter::repeat_n(HEIGHT, open));
    has_room(&h, wh)
}

pub(crate) fn slot_position(cx: &App, slot: usize) -> (f32, f32) {
    let (wx, wy, ww, wh) = work_area(cx);
    let mut h = heights();
    h.resize(slot + 1, HEIGHT);
    (
        wx + ww - WIDTH - MARGIN,
        stack_tops(wy + wh - MARGIN, &h)[slot],
    )
}
/// Перестроить стопку снизу вверх после закрытия или смены высоты (`relayout`).
pub(crate) fn relayout(cx: &mut App) {
    let handles: Vec<(AnyWindowHandle, f32)> = STATE
        .lock()
        .map(|inner| inner.active.iter().map(|a| (a.window, a.height)).collect())
        .unwrap_or_default();
    let mut places = Vec::with_capacity(handles.len());
    for (slot, (handle, h)) in handles.into_iter().enumerate() {
        let (x, y) = slot_position(cx, slot);
        if let Ok(hwnd) = handle.update(cx, |_, window, _| raw_hwnd(window)) {
            places.push((hwnd, x, y, h));
        }
    }
    // Вне занятого App: см. `place_window`.
    cx.spawn(async move |_| {
        for (hwnd, x, y, h) in places {
            place_window(hwnd, x, y, h);
        }
    })
    .detach();
    publish_overflow(cx);
}
/// Карточка измерила себя: окно принимает её высоту, стопка перекладывается.
pub fn set_height(id: u64, height: f32, cx: &mut App) {
    {
        let Ok(mut inner) = STATE.lock() else { return };
        let Some(active) = inner.active.iter_mut().find(|a| a.id == id) else {
            return;
        };
        if (active.height - height).abs() < 0.5 {
            return;
        }
        active.height = height;
    }
    // Размер окна ставит `relayout` вместе с местом (`move_window`).
    relayout(cx);
}

/// Бейдж «+N» показывает ВЕРХНИЙ тост, остальные — ничего.
pub(crate) fn publish_overflow(cx: &mut App) {
    let (views, queued) = {
        let Ok(inner) = STATE.lock() else { return };
        (
            inner
                .active
                .iter()
                .map(|a| a.view.clone())
                .collect::<Vec<_>>(),
            inner.queue.len(),
        )
    };
    let top = views.len().saturating_sub(1);
    for (i, view) in views.into_iter().enumerate() {
        let n = if i == top { queued } else { 0 };
        if let Some(view) = view.upgrade() {
            view.update(cx, |v, cx| {
                v.overflow = n;
                cx.notify();
            });
        }
    }
}
/// Поднять очередь в освободившиеся слоты (`pump_queue`).
pub(crate) fn pump_queue(cx: &mut App) {
    // По одному: каждое открытие заносит свою высоту, и место считается от неё.
    loop {
        let spec = {
            let Ok(inner) = STATE.lock() else { return };
            if inner.queue.is_empty() {
                None
            } else {
                Some(inner.active.len())
            }
        };
        let Some(slot) = spec else { break };
        if !room_for(cx, 0) {
            break;
        }
        let Some((id, opts)) = STATE.lock().ok().and_then(|mut inner| {
            let opts = inner.queue.pop_front()?;
            inner.next_id += 1;
            Some((inner.next_id, opts))
        }) else {
            break;
        };
        open_window(id, slot, opts, cx);
    }
    publish_overflow(cx);
}
/// Кадры тостам даёт ОДИН общий таск: у неактивного PopUp-окна gpui сам
/// кадры не планирует (`request_animation_frame` из его же render'а
/// молчит — полоса замирала на первом кадре, проверено замером).
pub(crate) fn ensure_ticker(cx: &mut App) {
    if TICKER.lock().map(|t| *t).unwrap_or(true) {
        return;
    }
    if let Ok(mut t) = TICKER.lock() {
        *t = true;
    }
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(33))
                .await;
            let handles: Vec<AnyWindowHandle> = STATE
                .lock()
                .map(|inner| inner.active.iter().map(|a| a.window).collect())
                .unwrap_or_default();
            if handles.is_empty() {
                break;
            }
            for handle in handles {
                let _ = cx.update_window(handle, |_, window, _| window.refresh());
            }
        }
        if let Ok(mut t) = TICKER.lock() {
            *t = false;
        }
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::{GAP, has_room, stack_tops};
    use crate::toast::{HEIGHT, MARGIN};

    #[test]
    fn taller_cards_push_the_ones_above_up() {
        let tops = stack_tops(1000.0, &[HEIGHT, 190.0, HEIGHT]);
        assert_eq!(tops[0], 1000.0 - HEIGHT);
        assert_eq!(tops[1], tops[0] - GAP - 190.0);
        assert_eq!(tops[2], tops[1] - GAP - HEIGHT);
    }

    #[test]
    fn room_counts_real_heights_not_a_fixed_step() {
        let work = 2.0 * MARGIN + 3.0 * (HEIGHT + GAP);
        assert!(has_room(&[HEIGHT, HEIGHT], work));
        assert!(!has_room(&[HEIGHT, 200.0], work));
        assert!(has_room(&[], 10.0), "the first toast always opens");
    }
}
