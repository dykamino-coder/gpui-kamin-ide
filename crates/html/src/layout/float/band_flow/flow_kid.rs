//! Блок потока в полосах (place_flow) и внутренняя ширина последовательности.

use super::kid::{Kid, Kind};
use super::{EPS, Slot, clearance, intrinsic, intrinsic_of, probe};
use crate::layout::float::bands::FloatBands;
use gpui::{App, Window};

#[allow(clippy::too_many_arguments)]
pub(super) fn place_flow(
    x0: f32,
    x1: f32,
    y0: f32,
    adjoining0: bool,
    bands: &mut FloatBands,
    window: &mut Window,
    cx: &mut App,
    cbw: f32,
    slots: &mut [Slot],
    y: &mut f32,
    k: usize,
    kid: &Kid,
    mt: f32,
    mr: f32,
    mb: f32,
    ml: f32,
) {
    // Коробка во всю ширину содержащего блока: флоаты её
    // перекрывают, а строки получают вырезы полос от её верха
    // (Servo `place_line_among_floats`, `inline/mod.rs:1466`;
    // Blink `ComputeLineLayoutOpportunity`,
    // `layout_opportunity.cc:164-189`). Вырезы меряются от краёв
    // САМОЙ коробки: стенки на время — её border-box.
    let mut top = clearance::top(
        bands,
        kid.clear,
        *y,
        mt,
        kid.start_open && adjoining0 && (*y - y0).abs() < EPS,
    );
    let avail = (cbw - ml - mr).max(0.0);
    // §9.5: «If a shortened line box is too small to contain any
    // content, then the line box is shifted downward … until either
    // some content fits or there are no more floats present».
    // Наборщик строк сдвигать строку вниз не умеет; у анонимного
    // прогона это то же, что опустить весь прогон до окна, в
    // которое влезает его самый узкий кусок (min-content, с
    // отступом первой строки): `below-float2/3` — флоат на всю
    // ширину, `x` с `text-indent` встаёт под ним, а не за краем.
    // Кусок — первое слово (`Kid::head`), когда оно известно.
    if kid.anon {
        let need = match kid.head.as_ref() {
            Some(h) => intrinsic_of(h, window, cx).0,
            None => intrinsic(kid, window, cx).0,
        };
        loop {
            let (l, r) = bands.available(top, 0.0);
            if r - l + EPS >= need.min(avail) {
                break;
            }
            match bands.next_edge(top) {
                Some(t) => top = t,
                None => break,
            }
        }
    }
    let walls = bands.set_walls(x0 + ml, x1 - mr);
    let shapes = bands.shapes(top);
    bands.set_walls(walls.0, walls.1);
    let (_, bh) = probe(kid, cbw, avail, Some(shapes.clone()), window, cx);
    slots[k] = Slot {
        x: x0 + ml,
        y: top,
        avail,
        shapes: Some(shapes),
        b: bh,
        ..Slot::default()
    };
    *y = top + bh + mb;
}

/// Внутренний размер контекста: min-content / max-content.
///
/// По Blink `BlockLayoutAlgorithm::ComputeMinMaxSizes`
/// (`block_layout_algorithm.cc:409-575`): флоаты копят инлайн-размер на
/// одной «строке» по сторонам, `clear` (у флоата или коробки своего
/// контекста) обрывает строку своей стороны, всякий не-флоат — обе; коробка
/// своего контекста прибавляет к себе отступы от флоатов рядом (поле
/// заменяет флоат, если больше); min-content — максимум по детям, каждый на
/// своей строке. Без обрыва на `clear` плавающий контейнер из дюжины
/// абзацев с `clear: left` мерился суммой всех (`letter-spacing-206-ref`).
pub(super) fn intrinsic_width(kids: &[Kid], max: bool, window: &mut Window, cx: &mut App) -> f32 {
    let (mut fl, mut fr) = (0.0f32, 0.0f32);
    let (mut max_size, mut min_size) = (0.0f32, 0.0f32);
    for kid in kids {
        if matches!(kid.kind, Kind::Strut(_)) {
            continue;
        }
        let [_, mr, _, ml] = kid.margin.map(|e| e.at(0.0));
        let (mn, mx) = match kid.nest.as_ref() {
            // Коробка с детьми на общих полосах: её внутренний размер — от
            // детей плюс рамка с отступом.
            Some(nest) => {
                let [_, ir, _, il] = nest.inset.map(|e| e.at(0.0));
                (
                    intrinsic_width(&nest.kids, false, window, cx) + il + ir,
                    intrinsic_width(&nest.kids, true, window, cx) + il + ir,
                )
            }
            None => intrinsic(kid, window, cx),
        };
        let is_float = matches!(kid.kind, Kind::Float { .. });
        let is_fc = matches!(kid.kind, Kind::Piece { .. });
        let clear = match kid.kind {
            Kind::Float { clear, .. } => clear,
            _ => kid.clear,
        };
        if is_float || is_fc {
            if clear.is_some() {
                max_size = max_size.max(fl + fr);
            }
            if matches!(clear, Some(0) | Some(-1)) {
                fl = 0.0;
            }
            if matches!(clear, Some(0) | Some(1)) {
                fr = 0.0;
            }
        }
        let contribution = match kid.kind {
            Kind::Float { side, .. } => {
                // Флоат целиком за краем содержимого (отрицательные поля) в
                // размер не входит.
                let f = mx + ml + mr;
                if f > 0.0 {
                    if side < 0 {
                        fl += f;
                    } else {
                        fr += f;
                    }
                }
                fl + fr
            }
            Kind::Piece { .. } => {
                let li = if ml > 0.0 { fl.max(ml) } else { fl + ml };
                let ri = if mr > 0.0 { fr.max(mr) } else { fr + mr };
                mx + li + ri
            }
            // Анонимный прогон строк — строчный контекст САМОГО хоста: флоаты
            // перед ним стоят в той же строке, и max-content строки — их сумма
            // с текстом (Blink `InlineNode::ComputeMinMaxSizes`, флоаты в
            // списке строчных элементов). Без суммы хост ужимался до ширины
            // флоата, текст уходил под него, и хост выходил вдвое выше
            // (`floats-122`: флоат `X` и `X` за ним — 50 вместо 100).
            _ if kid.anon => fl + fr + mx + ml + mr,
            _ => mx + ml + mr,
        };
        max_size = max_size.max(contribution);
        min_size = min_size.max(mn + ml + mr);
        if !is_float {
            fl = 0.0;
            fr = 0.0;
        }
    }
    if max { max_size } else { min_size }
}
