//! Хост полос обтекания с ИЗМЕРЕННЫМИ размерами (шаги F2, F3, F5 плана
//! `target/scout-float-bands-design.md`).
//!
//! Статический бандовый хост (`render.rs`, `shape_flow` с `bands=1`) знает
//! только литералы стиля: ширину содержащего блока точками и размеры каждого
//! флоата и куска хвоста точками. Здесь всё это берётся из раскладки:
//!
//! * ширина контекста — из замера (`known`/`available`) и из `bounds` в
//!   `prepaint`, а не из `style.width` (F2: `<td>` без ширины в стиле, тело
//!   без ширины);
//! * размер флоата — shrink-to-fit CSS 2.1 §10.3.5 пробной раскладкой ДО
//!   посадки (F3), как у Blink (`floats_utils.cc:173-189`,
//!   `LayoutFloatWithoutFragmentation`) и Servo (`flow/float.rs:899-919`
//!   `FloatBox::layout` → `place_float_fragment` `:1075-1137`);
//! * кусок хвоста со своим контекстом без размеров — ширина окна полос и
//!   высота из содержимого (F5, §9.5 последний абзац): перебор возможностей
//!   сверху вниз, как `BlockLayoutAlgorithm::LayoutNewFormattingContext`
//!   (Blink `block_layout_algorithm.cc:2092-2313`) и
//!   `layout_in_flow_block_level_sequentially` (Servo `flow/mod.rs:1335-1426`).
//!
//! Ограничение gpui: в замерном замыкании основной движок раскладки вынут из
//! окна. Пробная раскладка идёт на ОТДЕЛЬНОМ движке
//! (`Window::with_nested_layout`, KaminIDE patch), а элемент, разложенный
//! там, в основное дерево не годится — поэтому дети приходят не элементами, а
//! построителями, и строятся заново на каждую пробу и на `prepaint`.

use crate::bands::FloatBands;
use crate::flow::FloatShape;
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, Window, div, point, prelude::*, px, size,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

/// Вырезы обтекания для строк потокового ребёнка: левые и правые формы от
/// его верха (`FloatBands::shapes`).
pub type Shapes = Arc<(Vec<FloatShape>, Vec<FloatShape>)>;

/// Построитель ребёнка: `(ширина контекста, доступная ширина, вырезы,
/// высота содержимого) -> элемент`. Вызывается на каждую пробу заново (см.
/// доккомент модуля). Высоту получает только `Kind::Nest`: его коробка
/// строится БЕЗ детей и высотой из плана.
pub type Build = Rc<dyn Fn(f32, f32, Option<Shapes>, Option<f32>) -> AnyElement>;

/// Длина поля: точки или доля ширины содержащего блока (§8.3: проценты
/// полей — от ШИРИНЫ содержащего блока по обеим осям).
#[derive(Clone, Copy, Debug)]
pub enum Edge {
    Px(f32),
    Pct(f32),
}

impl Edge {
    fn at(self, cb: f32) -> f32 {
        match self {
            Edge::Px(v) => v,
            Edge::Pct(k) => k * cb,
        }
    }
}

/// Чем ребёнок хоста участвует в полосах.
#[derive(Clone, Copy, Debug)]
pub enum Kind {
    /// Флоат: сторона `-1`/`1` и `clear` из стиля; `shrink` — ширина
    /// `auto`, то есть shrink-to-fit §10.3.5.
    Float {
        side: i8,
        clear: Option<i8>,
        shrink: bool,
    },
    /// Коробка, флоаты НЕ перекрывающая (§9.5, последний абзац): свой
    /// контекст, таблица, атом известного размера. `table` — коробка не уже
    /// своего min-content (CSS 2.1 §17.5.2: ширина таблицы не меньше
    /// минимальной ширины ячеек), блок же со своим контекстом заполняет окно
    /// и при переполнении содержимым (§10.3.3).
    Piece { table: bool },
    /// Распорка: только высота margin-box, ничего не рисует.
    Strut(f32),
    /// Блок обычного потока (свой контекст НЕ заводит): коробка идёт во всю
    /// ширину поверх флоатов, а строки в ней сужаются (§9.5: «the current
    /// and subsequent line boxes created next to the float are shortened»).
    /// Канал — `FloatBands::shapes` от верха коробки (шаг F4).
    Flow,
    /// Блок обычного потока, внутри которого есть флоаты или коробки своего
    /// контекста (шаг F7): его дети раскладываются по ОБЩИМ полосам
    /// контекста со стенками его содержимого (Servo
    /// `ContainingBlockPositionInfo`, `flow/float.rs:44-64`, и
    /// `replace_containing_block_position_info`, `:971-977`; Blink — одно
    /// `ExclusionSpace` на БФК). Сама коробка рисуется без детей высотой из
    /// плана; детей несёт `Kid::nest`.
    Nest,
}

/// Содержимое `Kind::Nest`.
pub struct Nest {
    pub kids: Vec<Kid>,
    /// Рамка плюс отступ: верх, право, низ, лево.
    pub inset: [Edge; 4],
    /// Заданная высота содержимого в точках; `None` — из детей в потоке
    /// (§10.6.3: флоаты в высоту обычного блока не входят).
    pub height: Option<f32>,
    /// Заданная ширина содержимого в точках; `None` — на всю ширину
    /// содержащего блока (§10.3.3). `width: 0` — законный содержащий блок
    /// для флоатов (`letter-spacing-206`: `.squash {width: 0}` с дюжиной
    /// плавающих абзацев внутри).
    pub width: Option<f32>,
}

pub struct Kid {
    pub kind: Kind,
    /// `clear` куска или блока потока (шаг F6): верх его border-box не выше
    /// низа флоатов названной стороны (§9.5.2). У флоата `clear` живёт в
    /// `Kind::Float`.
    pub clear: Option<i8>,
    /// Поля: верх, право, низ, лево.
    pub margin: [Edge; 4],
    pub build: Build,
    /// Дети `Kind::Nest`.
    pub nest: Option<Nest>,
}

/// Место ребёнка в плане: левый верх border-box, доступная ширина (по ней
/// ребёнок строится заново в `prepaint`).
#[derive(Clone, Debug, Default)]
struct Slot {
    x: f32,
    y: f32,
    avail: f32,
    shapes: Option<Shapes>,
    /// Высота содержимого `Kind::Nest`.
    h: Option<f32>,
    /// Места детей `Kind::Nest`.
    kids: Vec<Slot>,
}

#[derive(Clone, Debug, Default)]
struct Plan {
    width: f32,
    height: f32,
    slots: Vec<Slot>,
}

/// Допуск сравнения точек — тот же, что у `bands.rs`.
const EPS: f32 = 0.01;

/// Отображение ребёнка в пробе: тот же самый каркас, что и в `prepaint`,
/// иначе проба и итог разойдутся.
///
/// Флоат — в колонке с `align-items: flex-start`: поперечный размер такого
/// элемента — fit-content = `min(max-content, max(min-content, доступно))`,
/// то есть ровно shrink-to-fit §10.3.5. Кусок — в колонке с растяжением:
/// `width: auto` у блока со своим контекстом заполняет окно (§10.3.3).
fn frame(kind: Kind, avail: f32, el: AnyElement, tap: Rc<Cell<Option<LayoutId>>>) -> AnyElement {
    let col = div().flex().flex_col().w(px(avail.max(0.0)));
    let col = if matches!(kind, Kind::Float { .. }) {
        col.items_start()
    } else {
        col
    };
    col.child(Tap { inner: el, id: tap }).into_any_element()
}

/// Пробная раскладка ребёнка при доступной ширине `avail`: размер border-box.
fn probe(
    kid: &Kid,
    cb: f32,
    avail: f32,
    shapes: Option<Shapes>,
    window: &mut Window,
    cx: &mut App,
) -> (f32, f32) {
    let tap = Rc::new(Cell::new(None));
    let mut el = frame(
        kid.kind,
        avail,
        (kid.build)(cb, avail, shapes, None),
        tap.clone(),
    );
    el.layout_as_root(
        size(
            AvailableSpace::Definite(px(avail.max(0.0))),
            AvailableSpace::MaxContent,
        ),
        window,
        cx,
    );
    unrounded(&tap, window)
}

/// Размер ребёнка по его `LayoutId` — БЕЗ округления к физической точке:
/// полосы складывают ширины, и округление каждой копится (`units-005`:
/// десять флоатов по `0.87em` в `8.7em`).
fn unrounded(tap: &Rc<Cell<Option<LayoutId>>>, window: &mut Window) -> (f32, f32) {
    match tap.get() {
        Some(id) => {
            let s = window.layout_size_unrounded(id);
            (f32::from(s.width), f32::from(s.height))
        }
        None => (0.0, 0.0),
    }
}

/// Внутренние ширины ребёнка (min-content, max-content) border-box.
fn intrinsic(kid: &Kid, window: &mut Window, cx: &mut App) -> (f32, f32) {
    let tap = Rc::new(Cell::new(None));
    let mut el = Tap {
        inner: (kid.build)(0.0, 0.0, None, None),
        id: tap.clone(),
    }
    .into_any_element();
    let mut w = |a: AvailableSpace, window: &mut Window| {
        el.layout_as_root(size(a, AvailableSpace::MaxContent), window, cx);
        unrounded(&tap, window).0
    };
    let mn = w(AvailableSpace::MinContent, window);
    let mx = w(AvailableSpace::MaxContent, window);
    (mn, mx.max(mn))
}

/// План раскладки при ширине контекста `cb`: позиции всех детей и высота.
fn plan(kids: &[Kid], cb: f32, window: &mut Window, cx: &mut App) -> Plan {
    let mut bands = FloatBands::new(cb);
    let (slots, y) = place_seq(kids, 0.0, cb, 0.0, true, (0.0, cb), &mut bands, window, cx);
    Plan {
        width: cb,
        // §10.6.7: хост охватывает флоаты — они абсолютные и высоту сами не
        // растят (как `min_h` статического хоста).
        height: bands.bottom(None).max(y),
        slots,
    }
}

/// Последовательная раскладка детей одного содержащего блока со стенками
/// `[x0, x1)` (координаты контекста) от высоты `y0`: места детей и низ
/// потока. Полосы — общие на весь контекст (Servo `SequentialLayoutState`,
/// `flow/float.rs:963`), стенки содержащего блока ставит вызывающий.
fn place_seq(
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
            } => {
                let mut avail = (cbw - ml - mr).max(0.0);
                // §10.3.5: shrink-to-fit = `min(max(min-content, доступно),
                // max-content)`. Каркас пробы (`align-items: flex-start`) даёт
                // `min(max-content, доступно)` — без пола min-content, и в
                // содержащем блоке нулевой ширины флоат схлопывался в ноль
                // (`white-space-intrinsic-size-001`: «the parent of the flow
                // is 0-width, so the float is min-content sized»). Пол — ширина
                // каркаса не уже min-content.
                if shrink {
                    avail = avail.max(intrinsic(kid, window, cx).0);
                }
                let (bw, bh) = probe(kid, cbw, avail, None, window, cx);
                // Правила 5 и 6 §9.5.1: флоат не выше низа предыдущего блока
                // потока (Servo `set_ceiling_from_non_floats`,
                // `flow/float.rs:371`). У пробега хоста `y` — ноль.
                bands.set_flow_ceiling(y);
                // Посадка margin-box: правила 1-9 §9.5.1 и clear §9.5.2 — в
                // `bands.add_float`.
                let (fx, fy) = bands.add_float(side, ml + bw + mr, mt + bh + mb, clear);
                slots[k] = Slot {
                    x: fx + ml,
                    y: fy + mt,
                    avail,
                    ..Slot::default()
                };
            }
            Kind::Strut(h) => {
                y += h;
            }
            Kind::Piece { table } => {
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
                let edge = |l: f32, r: f32| {
                    let (has_l, has_r) = (l > root.0 + EPS, r < root.1 - EPS);
                    let (ll, rr) = if !has_l && !has_r {
                        (l + ml, r - mr)
                    } else {
                        (l.max(x0 + ml.max(0.0)), r.min(x1 - mr.max(0.0)))
                    };
                    (ll, (rr - ll).max(0.0), has_l, has_r)
                };
                // Влезает ли border-box `bw` с левым краем `x` в окно `[l, r)`
                // (`:2259-2274`: не наезжать на флоаты слева и справа, а в
                // суженное окно — целиком).
                let fits = |l: f32, r: f32, x: f32, bw: f32, has_l: bool, has_r: bool| {
                    !(has_l && x < l - EPS)
                        && !(has_r && x + bw > r + EPS)
                        && !((has_l || has_r) && bw > r - l + EPS)
                };
                // Проба коробки с верхом `top`: место (левый край, доступная
                // ширина), если она влезает в окно на всю свою высоту. Окно на
                // верхней полосе → проба → проверка окна на всю высоту пробы
                // (`:2209`: блочный размер фрагмента не больше возможности);
                // ниже по высоте окно у́же — проба в нём ещё раз (Servo
                // `try_to_expand_for_auto_block_size`, `flow/float.rs:260`).
                let try_at = |top: f32, window: &mut Window, cx: &mut App| -> Option<(f32, f32)> {
                    let (l, r) = bands.available(top, 0.0);
                    let (x, avail, has_l, has_r) = edge(l, r);
                    let (bw, bh) = probe(kid, cbw, avail, None, window, cx);
                    let bw = bw.max(floor);
                    let (l2, r2) = bands.available(top, bh);
                    if (l2 - l).abs() < EPS && (r2 - r).abs() < EPS {
                        return fits(l, r, x, bw, has_l, has_r).then_some((x, avail));
                    }
                    let (x2, avail2, h_l, h_r) = edge(l2, r2);
                    let (bw2, bh2) = probe(kid, cbw, avail2, None, window, cx);
                    let bw2 = bw2.max(floor);
                    let (l3, r3) = bands.available(top, bh2);
                    (l3 <= l2 + EPS && r3 >= r2 - EPS && fits(l2, r2, x2, bw2, h_l, h_r))
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
                    ..Slot::default()
                };
                y = t + bh + mb;
            }
            Kind::Flow => {
                // Коробка во всю ширину содержащего блока: флоаты её
                // перекрывают, а строки получают вырезы полос от её верха
                // (Servo `place_line_among_floats`, `inline/mod.rs:1466`;
                // Blink `ComputeLineLayoutOpportunity`,
                // `layout_opportunity.cc:164-189`). Вырезы меряются от краёв
                // САМОЙ коробки: стенки на время — её border-box.
                let top = bands.clearance(kid.clear, y + mt);
                let avail = (cbw - ml - mr).max(0.0);
                let walls = bands.set_walls(x0 + ml, x1 - mr);
                let shapes = bands.shapes(top);
                bands.set_walls(walls.0, walls.1);
                let (_, bh) = probe(kid, cbw, avail, Some(shapes.clone()), window, cx);
                slots[k] = Slot {
                    x: x0 + ml,
                    y: top,
                    avail,
                    shapes: Some(shapes),
                    ..Slot::default()
                };
                y = top + bh + mb;
            }
            Kind::Nest => {
                let Some(nest) = kid.nest.as_ref() else {
                    continue;
                };
                let [it, ir, ib, il] = nest.inset.map(|e| e.at(cbw));
                let top = bands.clearance(kid.clear, y + mt);
                let (bx0, bx1) = (x0 + ml, x1 - mr);
                // Содержимое коробки — содержащий блок её детей (§10.1 п.2):
                // стенки полос на время детей.
                let (ix0, ix1) = match nest.width {
                    Some(w) => (bx0 + il, bx0 + il + w),
                    None => (bx0 + il, bx1 - ir),
                };
                let walls = bands.set_walls(ix0, ix1);
                let inner_top = top + it;
                // Дети примыкают, пока до коробки ничего не было, а её
                // верхний край открыт для схлопывания (нет рамки и отступа).
                let adjoining = adjoining0 && (y - y0).abs() < EPS && it == 0.0;
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
                    ..Slot::default()
                };
                y = top + it + h + ib + mb;
            }
        }
    }
    (slots, y)
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
fn intrinsic_width(kids: &[Kid], max: bool, window: &mut Window, cx: &mut App) -> f32 {
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

/// Дети и их места плоским списком в порядке краски: `floats == false` —
/// всё, кроме флоатов (коробка `Kind::Nest` раньше своих детей), `true` —
/// одни флоаты, на любой глубине.
fn flatten<'a>(
    kids: &'a [Kid],
    slots: &'a [Slot],
    floats: bool,
    out: &mut Vec<(&'a Kid, &'a Slot)>,
) {
    for (kid, s) in kids.iter().zip(slots.iter()) {
        let float = matches!(kid.kind, Kind::Float { .. });
        if float == floats {
            out.push((kid, s));
        }
        if let Some(nest) = kid.nest.as_ref() {
            flatten(&nest.kids, &s.kids, floats, out);
        }
    }
}

pub struct BandFlow {
    kids: Rc<Vec<Kid>>,
    plan: Rc<RefCell<Option<Plan>>>,
    built: Vec<AnyElement>,
}

impl BandFlow {
    pub fn new(kids: Vec<Kid>) -> Self {
        BandFlow {
            kids: Rc::new(kids),
            plan: Rc::new(RefCell::new(None)),
            built: Vec::new(),
        }
    }
}

impl Element for BandFlow {
    type RequestLayoutState = LayoutId;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        _cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        let kids = self.kids.clone();
        let cache = self.plan.clone();
        let mut style = gpui::Style::default();
        // Ширина `auto`, без сжатия: в колонке потока хост растягивается на
        // всю ширину (известную раскладке — `known.width`), а там, где
        // ширину решает содержимое (плавающий контейнер, строчный блок,
        // ячейка), — shrink-to-fit §10.3.5 от внутренних размеров детей.
        // Прежний `width: 100%` занимал всё доступное место и в пробе
        // флоата: плавающий `.contain` из одних плавающих абзацев выходил во
        // всю страницу вместо ширины самого широкого абзаца
        // (`letter-spacing-206-ref`).
        style.flex_shrink = 0.0;
        let id = window.request_measured_layout(style, move |known, available, window, cx| {
            let cb = match (known.width, available.width) {
                (Some(w), _) => f32::from(w),
                (None, AvailableSpace::Definite(w)) => {
                    let w = f32::from(w);
                    let (mn, mx) = window.with_nested_layout(|window| {
                        (
                            intrinsic_width(&kids, false, window, cx),
                            intrinsic_width(&kids, true, window, cx),
                        )
                    });
                    // §10.3.5: `min(max(min-content, available), max-content)`.
                    mx.max(mn).min(mn.max(w))
                }
                (None, a) => window.with_nested_layout(|window| {
                    intrinsic_width(&kids, matches!(a, AvailableSpace::MaxContent), window, cx)
                }),
            };
            if let Some(p) = cache.borrow().as_ref()
                && (p.width - cb).abs() < EPS
            {
                return size(px(cb), px(p.height));
            }
            let p = window.with_nested_layout(|window| plan(&kids, cb, window, cx));
            let h = p.height;
            *cache.borrow_mut() = Some(p);
            size(px(cb), px(h))
        });
        (id, id)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let _ = bounds;
        // Размер и начало хоста — БЕЗ округления к физической точке: дети
        // кладутся ОТДЕЛЬНЫМ деревом, и его края округляются от начала этого
        // дерева, а не от начала страницы (`taffy.rs` `layout_bounds`
        // округляет абсолютные края). Чтобы округление сошлось с основным
        // деревом, целая часть начала (в физических точках) уходит в
        // смещение дерева, а дробная — в позиции детей: round(n + f) = n +
        // round(f) (`float-nowrap-hyphen-rewind-1-ref2` при масштабе 1.25:
        // хост на 9.0 логических = 11.25 физических, его текст на 12.5 →
        // 13 в основном дереве, но 1.25 → 1 во вложенном — на точку левее и
        // выше соседей).
        let cb = f32::from(window.layout_size_unrounded(*state).width);
        let origin = window.layout_origin_unrounded(*state);
        let scale = window.scale_factor();
        let (ox, oy) = (f32::from(origin.x) * scale, f32::from(origin.y) * scale);
        let (ix, iy) = (ox.floor(), oy.floor());
        let (fx, fy) = ((ox - ix) / scale, (oy - iy) / scale);
        let origin = point(px(ix / scale), px(iy / scale));
        let cached = self
            .plan
            .borrow()
            .as_ref()
            .filter(|p| (p.width - cb).abs() < EPS)
            .cloned();
        let p = match cached {
            Some(p) => p,
            None => {
                let kids = self.kids.clone();
                window.with_nested_layout(|window| plan(&kids, cb, window, cx))
            }
        };
        self.built.clear();
        // Все дети — ОДНИМ корнем: относительный каркас и абсолютные
        // держатели по местам плана, как у статического хоста. Округление к
        // физической точке идёт на абсолютных краях внутри одного дерева
        // (`taffy.rs` `layout_bounds`), и соседние флоаты сходятся без щелей;
        // корень на каждого ребёнка округлял бы размер отдельно от места
        // (`units-005`: сто флоатов по `0.87em` с красными швами).
        // Порядок отрисовки — порядок детей: флоаты пробега, потом хвост.
        let mut host = div().relative().w(px(cb + fx)).h(px(p.height + fy));
        // CSS 2.1 прил. E: фоны блоков потока (шаг 4) — РАНЬШЕ флоатов
        // (шаг 5): флоат лежит поверх блока, под которым стоит
        // (`clear-004`). Строки рядом с флоатом его не перекрывают — их
        // порядок с флоатом не виден.
        let mut flat: Vec<(&Kid, &Slot)> = Vec::new();
        flatten(&self.kids, &p.slots, false, &mut flat);
        flatten(&self.kids, &p.slots, true, &mut flat);
        for (kid, s) in flat {
            if matches!(kid.kind, Kind::Strut(_)) {
                continue;
            }
            let tap = Rc::new(Cell::new(None));
            let el = frame(
                kid.kind,
                s.avail,
                (kid.build)(cb, s.avail, s.shapes.clone(), s.h),
                tap,
            );
            host = host.child(
                div()
                    .absolute()
                    .left(px(s.x + fx))
                    .top(px(s.y + fy))
                    .child(el),
            );
        }
        let mut el = host.into_any_element();
        el.layout_as_root(
            size(
                AvailableSpace::Definite(px(cb + fx)),
                AvailableSpace::Definite(px(p.height + fy)),
            ),
            window,
            cx,
        );
        el.prepaint_at(origin, window, cx);
        self.built.push(el);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut LayoutId,
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        for el in self.built.iter_mut() {
            el.paint(window, cx);
        }
    }
}

impl IntoElement for BandFlow {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Прозрачная обёртка, запоминающая `LayoutId` ребёнка: после пробной
/// раскладки каркаса по нему читается размер самого ребёнка, а не каркаса.
struct Tap {
    inner: AnyElement,
    id: Rc<Cell<Option<LayoutId>>>,
}

impl Element for Tap {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let id = self.inner.request_layout(window, cx);
        self.id.set(Some(id));
        (id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.inner.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.inner.paint(window, cx);
    }
}

impl IntoElement for Tap {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
