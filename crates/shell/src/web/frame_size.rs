//! Размер кадра, который просим у CEF, и когда кадр «не того размера».
//!
//! Под драгом ручки размер для CEF квантуется вверх до шага: каждый
//! `was_resized` — полная перевёрстка страницы. Отрисовка панели при этом
//! сверяет пришедшую текстуру с размером и при расхождении подталкивает CEF
//! (`browsers::nudge`: `was_resized` + `invalidate`). Сверка шла с ТОЧНЫМ
//! размером панели — квантованный кадр законно шире, расхождение видно на
//! каждом кадре окна, и nudge на каждом кадре заставлял оба вью
//! перерисовываться с частотой окна весь драг: ~119 кадров CEF/с против
//! ~48 после правки (INC-2026-0001, замер драга). Поэтому шаг, запрошенный
//! размер и решение о подталкивании живут здесь вместе: сверяем с тем, что
//! попросили, и не трогаем запрос, который ещё в пути.

use std::time::{Duration, Instant};

/// Шаг квантования размера ПОД ДРАГОМ ручки. Слали ~30 команд/с, и каждая
/// ОТМЕНЯЛА незавершённую перевёрстку: CEF не «не успевал», он бесконечно
/// начинал заново, а кадры приходили чужого размера. С квантованием за драг
/// уходит 3-4 команды, и каждая доводится до конца.
const DRAG_STEP_PX: i32 = 32;

/// Сколько ждём кадр нового размера после `was_resized`, прежде чем считать
/// запрос потерянным. Перевёрстка страницы — десятки мс.
const IN_FLIGHT: Duration = Duration::from_millis(120);

/// Расхождение текстуры с запрошенным размером, которое не считается.
const SLACK_PX: i32 = 2;

/// Размер для CEF: под драгом — вверх до шага (кадр не меньше панели),
/// иначе точный; точный уходит и по отпусканию ручки.
pub(crate) fn for_drag(width: i32, height: i32) -> (i32, i32) {
    if crate::state::drag_flag::is_dragging() {
        (quantize(width), quantize(height))
    } else {
        (width, height)
    }
}

fn quantize(v: i32) -> i32 {
    ((v + DRAG_STEP_PX - 1) / DRAG_STEP_PX) * DRAG_STEP_PX
}

/// Текстура вью не того размера, что у CEF попросили. `exact` — точный
/// размер панели в физических px: он же запрос до первого `resize`.
pub(crate) fn mismatch(id: &str, texture: (i32, i32), exact: (i32, i32), scale: f32) -> bool {
    differs(texture, asked_px(id, exact, scale))
}

/// Запрошенный у CEF размер в физических px.
fn asked_px(id: &str, exact: (i32, i32), scale: f32) -> (i32, i32) {
    super::browsers::wanted_size(id).map_or(exact, |(w, h)| {
        (
            (w as f32 * scale).round() as i32,
            (h as f32 * scale).round() as i32,
        )
    })
}

/// Текстура не того размера, что попросили.
fn differs(texture: (i32, i32), asked: (i32, i32)) -> bool {
    (texture.0 - asked.0).abs() > SLACK_PX || (texture.1 - asked.1).abs() > SLACK_PX
}

/// Запрос размера ещё в пути: отложен троттлингом или ушёл недавно.
/// `pending` — (ждёт отправки, когда отправлен) из `browsers`.
fn in_flight(pending: Option<(bool, Instant)>, now: Instant) -> bool {
    pending.is_some_and(|(dirty, sent)| dirty || now.duration_since(sent) < IN_FLIGHT)
}

/// Кадр не того размера: подтолкнуть CEF, только если запрос потерян.
///
/// Пока запрос в пути, кадр нужного размера ещё едет — просим лишь ещё один
/// кадр окна, чтобы сверка повторилась и после окна ожидания: потерянный
/// запрос по-прежнему подталкивается, просто не раньше `IN_FLIGHT`.
pub(crate) fn nudge_if_lost(id: &str) {
    if in_flight(super::browsers::resize_pending(id), Instant::now()) {
        super::repaint_requested();
    } else {
        super::browsers::nudge(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_size_rounds_up_to_the_step() {
        assert_eq!(quantize(1), 32);
        assert_eq!(quantize(32), 32);
        assert_eq!(quantize(33), 64);
        assert_eq!(quantize(457), 480);
    }

    #[test]
    fn a_quantized_texture_matches_what_was_asked() {
        // Панель 457 px, попросили 480: кадр 480 — не «потерянный запрос».
        assert!(!differs((480, 746), (480, 746)));
        assert!(!differs((482, 744), (480, 746)));
        assert!(differs((457, 746), (480, 746)));
    }

    #[test]
    fn only_a_request_that_had_time_to_arrive_counts_as_lost() {
        let now = Instant::now();
        let sent = |ms| now - Duration::from_millis(ms);
        assert!(in_flight(Some((true, sent(500))), now), "ещё не отправлен");
        assert!(in_flight(Some((false, sent(10))), now), "только что ушёл");
        assert!(!in_flight(Some((false, sent(500))), now), "давно ушёл");
        assert!(!in_flight(None, now), "вью ни разу не ресайзили");
    }
}
