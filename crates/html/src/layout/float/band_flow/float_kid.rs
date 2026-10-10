//! Флоат и вложенный поток в полосах: ширина shrink-to-fit, потолок потока, вставка в полосы.

use super::kid::{Kid, Kind};
use super::place::place_seq;
use super::{Build, EPS, Slot, clearance, intrinsic, intrinsic_of, probe, probe_of};
use crate::layout::float::band_flow::VERT;
use crate::layout::float::bands::FloatBands;
use gpui::{App, Window};
use std::cell::Cell;
use std::ops::ControlFlow;

#[allow(clippy::too_many_arguments)]
pub(super) fn place_float(
    x0: f32,
    x1: f32,
    bands: &mut FloatBands,
    window: &mut Window,
    cx: &mut App,
    cbw: f32,
    slots: &mut [Slot],
    y: f32,
    k: usize,
    kid: &Kid,
    mt: f32,
    mr: f32,
    mb: f32,
    ml: f32,
    side: i8,
    clear: Option<i8>,
    shrink: bool,
    letter: bool,
) {
    let mut avail = (cbw - ml - mr).max(0.0);
    // §10.3.5: shrink-to-fit = `min(max(min-content, доступно),
    // max-content)`. Каркас пробы (`align-items: flex-start`) даёт
    // `min(max-content, доступно)` — без пола min-content, и в
    // содержащем блоке нулевой ширины флоат схлопывался в ноль
    // (`white-space-intrinsic-size-001`: «the parent of the flow
    // is 0-width, so the float is min-content sized»). Пол — ширина
    // каркаса не уже min-content.
    if shrink {
        let (mn, mx) = intrinsic(kid, window, cx);
        avail = if VERT.with(Cell::get).is_some() {
            // В вертикальном письме каркас — ряд, и поперечная ось
            // ряда (высота) у вертикального блока растягивается
            // до окна и при `align-items: flex-start`: строчный
            // размер флоата задаётся каркасу явно, полной формулой
            // §10.3.5 `min(max(min-content, доступно), max-content)`.
            mx.min(mn.max(avail))
        } else {
            avail.max(mn)
        };
    }
    let (bw, bh) = probe(kid, cbw, avail, None, window, cx);
    // Правила 5 и 6 §9.5.1: флоат не выше низа предыдущего блока
    // потока (Servo `set_ceiling_from_non_floats`,
    // `flow/float.rs:371`). У пробега хоста `y` — ноль.
    // Флоат посреди строки (правило 6 §9.5.1, Blink
    // `NGLineBreaker::HandleFloat`: флоат встаёт на текущую
    // строку, если влезает в её остаток рядом с уже набранным,
    // иначе — под неё). Набранное до флоата — `Kid::lead`; не
    // влезло рядом — потолок опускается на его высоту.
    let mut ceil = y + kid.margin_offset;
    // Высота набранного — с вырезами уже поставленных флоатов:
    // строки рядом с ними у́же и их больше.
    let height_of = |b: &Build, bands: &mut FloatBands, window: &mut Window, cx: &mut App| {
        let walls = bands.set_walls(x0, x1);
        let shapes = bands.shapes(y);
        bands.set_walls(walls.0, walls.1);
        probe_of(Kind::Flow, b, cbw, cbw, Some(shapes), window, cx).1
    };
    // Строка флоата начинается под набранным до последнего
    // `<br>` (`floats-placement-vertical-004-ref`: «H<br>» и
    // флоат на второй строке рядом с первым флоатом).
    if let Some(base) = kid.lead_base.as_ref() {
        ceil = y + height_of(base, bands, window, cx);
    }
    if let Some(lead) = kid.lead.as_ref() {
        let lw = intrinsic_of(lead, window, cx).1;
        let (l, r) = bands.available(ceil, 0.0);
        if lw > EPS && lw + ml + bw + mr > r - l + EPS {
            ceil += height_of(lead, bands, window, cx);
        }
    }
    bands.set_flow_ceiling(ceil);
    // Посадка margin-box: правила 1-9 §9.5.1 и clear §9.5.2 — в
    // `bands.add_float`.
    let (fx, fy) = if letter {
        bands.add_initial_letter(side, ml + bw + mr, mt + bh + mb, y)
    } else {
        bands.add_float(side, ml + bw + mr, mt + bh + mb, clear)
    };
    slots[k] = Slot {
        x: fx + ml,
        y: fy + mt,
        avail,
        b: bh,
        ..Slot::default()
    };
}

#[allow(clippy::too_many_arguments)]
pub(super) fn place_nest(
    x0: f32,
    x1: f32,
    y0: f32,
    adjoining0: bool,
    root: (f32, f32),
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
) -> ControlFlow<()> {
    let Some(nest) = kid.nest.as_ref() else {
        return ControlFlow::Break(());
    };
    let [it, ir, ib, il] = nest.inset.map(|e| e.at(cbw));
    let top = clearance::top(
        bands,
        kid.clear,
        *y,
        mt,
        kid.start_open && adjoining0 && (*y - y0).abs() < EPS,
    );
    let (bx0, bx1) = (x0 + ml, x1 - mr);
    let (ix0, ix1) = match nest.width {
        Some(w) => (bx0 + il, bx0 + il + w),
        None => (bx0 + il, bx1 - ir),
    };
    let walls = bands.set_walls(ix0, ix1);
    let inner_top = top + it;
    let adjoining = adjoining0 && (*y - y0).abs() < EPS && it == 0.0;
    let (kids_slots, inner_end) = place_seq(
        &nest.kids, ix0, ix1, inner_top, adjoining, root, bands, window, cx,
    );
    bands.set_walls(walls.0, walls.1);
    let h = nest.height.unwrap_or((inner_end - inner_top).max(0.0));
    slots[k] = Slot {
        x: bx0,
        y: top,
        avail: (bx1 - bx0).max(0.0),
        h: Some(h),
        kids: kids_slots,
        b: it + h + ib,
        ..Slot::default()
    };
    *y = top + it + h + ib + mb;
    // Содержимое коробки — содержащий блок её детей (§10.1 п.2):
    // стенки полос на время детей.
    // Дети примыкают, пока до коробки ничего не было, а её
    // верхний край открыт для схлопывания (нет рамки и отступа).
    ControlFlow::Continue(())
}
