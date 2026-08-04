//! Стопка тостов: слоты, раскладка, очередь и переполнение.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::toast::STATE;
use crate::toast::TICKER;
use crate::toast::open_window;
use crate::toast::win32::move_window;
use crate::toast::win32::work_area;
use crate::toast::{HARD_MAX_VISIBLE, HEIGHT, MARGIN, STEP, WIDTH};
use gpui::prelude::*;
use gpui::{AnyWindowHandle, App};

/// Сколько тостов помещается по высоте (`max_visible`).
pub(crate) fn max_visible(cx: &App) -> usize {
    let (_, _, _, wh) = work_area(cx);
    let fits = ((wh - 2.0 * MARGIN) / STEP).floor().max(1.0) as usize;
    HARD_MAX_VISIBLE.min(fits)
}
pub(crate) fn slot_position(cx: &App, slot: usize) -> (f32, f32) {
    let (wx, wy, ww, wh) = work_area(cx);
    (
        wx + ww - WIDTH - MARGIN,
        wy + wh - HEIGHT - MARGIN - (slot as f32) * STEP,
    )
}
/// Перестроить стопку снизу вверх после закрытия (`relayout`).
pub(crate) fn relayout(cx: &mut App) {
    let handles: Vec<AnyWindowHandle> = STATE
        .lock()
        .map(|inner| inner.active.iter().map(|a| a.window).collect())
        .unwrap_or_default();
    for (slot, handle) in handles.into_iter().enumerate() {
        let (x, y) = slot_position(cx, slot);
        let _ = handle.update(cx, |_, window, _| move_window(window, x, y));
    }
    publish_overflow(cx);
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
    let max = max_visible(cx);
    let specs = {
        let Ok(mut inner) = STATE.lock() else { return };
        let mut specs = Vec::new();
        while inner.active.len() + specs.len() < max {
            let Some(opts) = inner.queue.pop_front() else {
                break;
            };
            inner.next_id += 1;
            specs.push((inner.next_id, inner.active.len() + specs.len(), opts));
        }
        specs
    };
    for (id, slot, opts) in specs {
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
