//! Расстановка детей потока по полосам занятости: обход последовательности (place_seq).

use super::kid::{Kid, Kind};
use super::{EPS, Slot, intrinsic, piece, place_float, place_flow, place_nest, probe};
use crate::layout::float::bands::FloatBands;
use gpui::{App, Window};
use std::ops::ControlFlow;

/// Последовательная раскладка детей одного содержащего блока со стенками
/// `[x0, x1)` (координаты контекста) от высоты `y0`: места детей и низ
/// потока. Полосы — общие на весь контекст (Servo `SequentialLayoutState`,
/// `flow/float.rs:963`), стенки содержащего блока ставит вызывающий.
#[allow(clippy::too_many_arguments)]
pub(super) fn place_seq(
    kids: &[Kid],
    x0: f32,
    x1: f32,
    y0: f32,
    // До `y0` в контексте ещё не было поточного содержимого: поля первых
    // детей схлопываются с верхом контекста, флоаты до них — примыкающие.
    adjoining0: bool,
    // Стенки КОНТЕКСТА (корня хоста): по ним узнаётся, суживают ли окно
    // флоаты, когда край флоата совпал со стенкой вложенного содержащего
    // блока.
    root: (f32, f32),
    bands: &mut FloatBands,
    window: &mut Window,
    cx: &mut App,
) -> (Vec<Slot>, f32) {
    let cbw = (x1 - x0).max(0.0);
    let mut slots = vec![Slot::default(); kids.len()];
    // Потолок потока: низ предыдущего куска плюс его нижнее поле (правило 5
    // §9.5.1 для кусков: окно ищется не выше).
    let mut y = y0;
    for (k, kid) in kids.iter().enumerate() {
        let [mt, mr, mb, ml] = kid.margin.map(|e| e.at(cbw));
        match kid.kind {
            Kind::Float {
                side,
                clear,
                shrink,
                letter,
            } => {
                place_float(
                    x0, x1, bands, window, cx, cbw, &mut slots, y, k, kid, mt, mr, mb, ml, side,
                    clear, shrink, letter,
                );
            }
            Kind::Strut(h) => {
                y += h;
            }
            Kind::Piece { table, rtl } => {
                // Пол ширины таблицы: каркас пробы жмёт её до окна, а
                // переполнение содержимым ширины коробки не растит.
                let floor = if table {
                    intrinsic(kid, window, cx).0
                } else {
                    0.0
                };
                // Окно `[l, r)` → (левый край коробки, доступная ширина) по
                // Blink `block_layout_algorithm.cc:2136-2178`: окно, не
                // суженное флоатами, урезается полями; суженное — нет, поля
                // откладываются от края СОДЕРЖАЩЕГО БЛОКА («Margins are
                // applied from the content-box, not the layout opportunity
                // area»), и окно лишь сжимается, если поле длиннее флоата.
                // Сужение узнаётся сравнением краёв окна со стенками
                // КОНТЕКСТА (Blink `:2136-2141`: «We can detect this when the
                // opportunity-rect sides match the available-rect sides»).
                // Флоат нулевой ширины у самой стенки окно не сужает
                // (`zero-width-floats`: коробка с полями `0 -50px` уходит за
                // стенки); флоат, чей край совпал со стенкой ВЛОЖЕННОГО
                // содержащего блока, сужает (`floats-wrap-bfc-with-margin-008`:
                // правый флоат 50 в блоке 100, содержащий блок коробки —
                // `margin-right: 50px`).
                // Проба коробки с верхом `top`: место (левый край, доступная
                // ширина), если она влезает в окно на всю свою высоту. Окно на
                // верхней полосе → проба → проверка окна на всю высоту пробы
                // (`:2209`: блочный размер фрагмента не больше возможности);
                // ниже по высоте окно у́же — проба в нём ещё раз (Servo
                // `try_to_expand_for_auto_block_size`, `flow/float.rs:260`).
                let try_at = |top: f32, window: &mut Window, cx: &mut App| -> Option<(f32, f32)> {
                    let (l, r) = bands.available(top, 0.0);
                    let edge = piece::Edges::new(l, r, root, (x0, x1), (ml, mr));
                    let avail = edge.available();
                    let (bw, bh) = probe(kid, cbw, avail, None, window, cx);
                    let bw = bw.max(floor);
                    let x = edge.origin(bw, rtl);
                    let (l2, r2) = bands.available(top, bh);
                    if (l2 - l).abs() < EPS && (r2 - r).abs() < EPS {
                        return edge.fits(l, r, x, bw).then_some((x, avail));
                    }
                    let edge2 = piece::Edges::new(l2, r2, root, (x0, x1), (ml, mr));
                    let avail2 = edge2.available();
                    let (bw2, bh2) = probe(kid, cbw, avail2, None, window, cx);
                    let bw2 = bw2.max(floor);
                    let x2 = edge2.origin(bw2, rtl);
                    let (l3, r3) = bands.available(top, bh2);
                    (l3 <= l2 + EPS && r3 >= r2 - EPS && edge2.fits(l2, r2, x2, bw2))
                        .then_some((x2, avail2))
                };
                // Примыкающие флоаты (Blink `block_layout_algorithm.cc`
                // `HasClearancePastAdjoiningFloats`; для нового контекста —
                // перезапуск с разрешённым смещением): в потоке до коробки ещё
                // ничего не было, её верхнее поле схлопывается до самого верха
                // контекста, и флоаты стоят там же, где началась бы коробка без
                // поля. Если рядом с ними ей нет места, поле ОТДЕЛЯЕТСЯ от
                // флоатов, как clearance, и коробка встаёт сразу под ними
                // (`new-fc-separates-from-float-2`: `margin-top: 12345px` при
                // флоате 200 из 200 — коробка на низе флоата; ассерт теста:
                // «will need to separate its margin from the float, so that it
                // doesn't affect the float»). Влезает — поле действует как есть.
                let adjoining = adjoining0 && (y - y0).abs() < EPS && mt > 0.0;
                let mt_eff = if adjoining && try_at(y, window, cx).is_none() {
                    0.0
                } else {
                    mt
                };
                // §9.5, последний абзац: border-box куска не перекрывает
                // margin-box флоатов. Перебор окон сверху вниз. §9.5.2:
                // гипотетическая позиция — `y + mt` (поля уже схлопнуты на
                // уровне узлов); не ниже флоатов — clearance ставит верх рамки
                // ровно на их низ (Blink `AdjustToClearance`,
                // `space_utils.cc:22-30`).
                let mut top = bands.clearance(kid.clear, y + mt_eff);
                let (x, t, avail) = loop {
                    if let Some((x, avail)) = try_at(top, window, cx) {
                        break (x, top, avail);
                    }
                    match bands.next_edge(top) {
                        Some(t) => top = t,
                        // Ниже всех флоатов — на всю ширину.
                        None => break (x0 + ml, top, (cbw - ml - mr).max(0.0)),
                    }
                };
                let (_, bh) = probe(kid, cbw, avail, None, window, cx);
                slots[k] = Slot {
                    x,
                    y: t,
                    avail,
                    b: bh,
                    ..Slot::default()
                };
                y = t + bh + mb;
            }
            Kind::Flow => {
                place_flow(
                    x0, x1, y0, adjoining0, bands, window, cx, cbw, &mut slots, &mut y, k, kid, mt,
                    mr, mb, ml,
                );
            }
            Kind::Nest => {
                if let ControlFlow::Break(_) = place_nest(
                    x0, x1, y0, adjoining0, root, bands, window, cx, cbw, &mut slots, &mut y, k,
                    kid, mt, mr, mb, ml,
                ) {
                    continue;
                }
            }
        }
    }
    (slots, y)
}
