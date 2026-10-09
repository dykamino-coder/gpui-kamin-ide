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
mod clearance;
mod piece;
mod kid;
pub use kid::{Kid, Kind, Nest};
use crate::layout::float::shapes::FloatShape;
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
    /// Блочный размер border-box: в вертикальном письме по нему ставится
    /// физический левый край (`vertical-rl` идёт от ПРАВОГО края).
    b: f32,
}

thread_local! {
    /// Письмо хоста на время плана и сборки: `None` — горизонтальное,
    /// `Some(true)` — `vertical-rl`, `Some(false)` — `vertical-lr` (шаг F10).
    /// Полосы и весь план считают в ЛОГИЧЕСКИХ осях (inline = вертикаль,
    /// block = горизонталь) — как Blink: `BfcOffset` строчно-относителен
    /// (`geometry/bfc_offset.h:26-38`), в физику переводится один раз
    /// (`geometry/writing_mode_converter.cc:80-102`). Каркас пробы и
    /// держатели в `prepaint` — в физике, по этому признаку.
    static VERT: Cell<Option<bool>> = const { Cell::new(None) };
}

/// Поставить письмо хоста на время `f`.
fn with_vert<R>(v: Option<bool>, f: impl FnOnce() -> R) -> R {
    let prev = VERT.with(|c| c.replace(v));
    let r = f();
    VERT.with(|c| c.set(prev));
    r
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
    // Вертикальное письмо: строчная ось — физическая высота, каркас — ряд
    // высотой в окно, поперечная ось ряда и есть строчная (css-writing-modes-4
    // §7.1: shrink-to-fit §10.3.5 считается по строчной оси).
    let col = if VERT.with(Cell::get).is_some() {
        div().flex().flex_row().h(px(avail.max(0.0)))
    } else {
        div().flex().flex_col().w(px(avail.max(0.0)))
    };
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
    probe_of(kid.kind, &kid.build, cb, avail, shapes, window, cx)
}

/// `probe` для любого построителя: каркас по виду `kind`.
fn probe_of(
    kind: Kind,
    build: &Build,
    cb: f32,
    avail: f32,
    shapes: Option<Shapes>,
    window: &mut Window,
    cx: &mut App,
) -> (f32, f32) {
    let tap = Rc::new(Cell::new(None));
    let mut el = frame(kind, avail, build(cb, avail, shapes, None), tap.clone());
    let vert = VERT.with(Cell::get).is_some();
    let room = AvailableSpace::Definite(px(avail.max(0.0)));
    let space = if vert {
        size(AvailableSpace::MaxContent, room)
    } else {
        size(room, AvailableSpace::MaxContent)
    };
    el.layout_as_root(space, window, cx);
    // (строчный, блочный) размер: в вертикальном письме — (высота, ширина).
    let (w, h) = unrounded(&tap, window);
    if vert { (h, w) } else { (w, h) }
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
    intrinsic_of(&kid.build, window, cx)
}

/// Внутренние ширины того, что строит `build` (ребёнок или голова прогона).
fn intrinsic_of(build: &Build, window: &mut Window, cx: &mut App) -> (f32, f32) {
    let tap = Rc::new(Cell::new(None));
    let mut el = Tap {
        inner: build(0.0, 0.0, None, None),
        id: tap.clone(),
    }
    .into_any_element();
    let vert = VERT.with(Cell::get).is_some();
    let mut w = |a: AvailableSpace, window: &mut Window| {
        if vert {
            // Повёрнутый текст свою длину высотой не заявляет (растягивается
            // окном) — берём её у самого текста через сборщик
            // `VT_INLINE_MAX`; коробка без текста отвечает раскладкой.
            let prev = crate::text::vertical::VT_INLINE_MAX.with(|c| c.replace(Some(0.0)));
            el.layout_as_root(size(AvailableSpace::MaxContent, a), window, cx);
            let text = crate::text::vertical::VT_INLINE_MAX
                .with(|c| c.replace(prev))
                .unwrap_or(0.0);
            let laid = unrounded(&tap, window).1;
            if text > 0.0 { text.min(laid) } else { laid }
        } else {
            el.layout_as_root(size(a, AvailableSpace::MaxContent), window, cx);
            unrounded(&tap, window).0
        }
    };
    let mn = w(AvailableSpace::MinContent, window);
    let mx = w(AvailableSpace::MaxContent, window);
    (mn, mx.max(mn))
}

/// План раскладки при ширине контекста `cb`: позиции всех детей и высота.
fn plan(kids: &[Kid], cb: f32, contain_floats: bool, window: &mut Window, cx: &mut App) -> Plan {
    let mut bands = FloatBands::new(cb);
    let (slots, y) = place_seq(kids, 0.0, cb, 0.0, true, (0.0, cb), &mut bands, window, cx);
    Plan {
        width: cb,
        // §§10.6.3, 10.6.7: only a formatting-context root contains floats.
        height: if contain_floats { bands.bottom(None).max(y) } else { y },
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
                letter,
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
                let height_of = |b: &Build,
                                 bands: &mut FloatBands,
                                 window: &mut Window,
                                 cx: &mut App| {
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
                // Коробка во всю ширину содержащего блока: флоаты её
                // перекрывают, а строки получают вырезы полос от её верха
                // (Servo `place_line_among_floats`, `inline/mod.rs:1466`;
                // Blink `ComputeLineLayoutOpportunity`,
                // `layout_opportunity.cc:164-189`). Вырезы меряются от краёв
                // САМОЙ коробки: стенки на время — её border-box.
                let mut top = clearance::top(
                    bands,
                    kid.clear,
                    y,
                    mt,
                    kid.start_open && adjoining0 && (y - y0).abs() < EPS,
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
                y = top + bh + mb;
            }
            Kind::Nest => {
                let Some(nest) = kid.nest.as_ref() else {
                    continue;
                };
                let [it, ir, ib, il] = nest.inset.map(|e| e.at(cbw));
                let top = clearance::top(
                    bands,
                    kid.clear,
                    y,
                    mt,
                    kid.start_open && adjoining0 && (y - y0).abs() < EPS,
                );
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
                    b: it + h + ib,
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
    /// Письмо хоста (см. `VERT`).
    vert: Option<bool>,
    contain_floats: bool,
}

impl BandFlow {
    pub fn new(kids: Vec<Kid>, contain_floats: bool) -> Self {
        BandFlow {
            kids: Rc::new(kids),
            plan: Rc::new(RefCell::new(None)),
            built: Vec::new(),
            vert: None,
            contain_floats,
        }
    }

    /// Вертикальное письмо хоста: `rl` — `vertical-rl`, иначе `vertical-lr`.
    pub fn vertical(mut self, rl: bool) -> Self {
        self.vert = Some(rl);
        self
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
        // Auto inline size stretches when layout supplies known.width; float,
        // inline-block and cell hosts use their children's shrink-to-fit sizes
        // (CSS 2.1 §10.3.5). A percentage width would instead fill all available
        // space even during intrinsic measurement: the floating .contain in
        // letter-spacing-206-ref must fit its widest floated paragraph.
        // Height follows the owner's float-containment policy independently
        // of this inline-size calculation (§§10.6.3, 10.6.7).
        style.flex_shrink = 0.0;
        let (vert, contain_floats) = (self.vert, self.contain_floats);
        let id = window.request_measured_layout(style, move |known, available, window, cx| {
            with_vert(vert, || {
                // Строчная ось — ширина в горизонтальном письме, высота в
                // вертикальном; блочный размер плана уходит в другую ось.
                let (known_inline, avail_inline) = if vert.is_some() {
                    (known.height, available.height)
                } else {
                    (known.width, available.width)
                };
                let phys = |inline: f32, block: f32| {
                    if vert.is_some() {
                        size(px(block), px(inline))
                    } else {
                        size(px(inline), px(block))
                    }
                };
                let cb = match (known_inline, avail_inline) {
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
                    return phys(cb, p.height);
                }
                let p = window.with_nested_layout(|window| plan(&kids, cb, contain_floats, window, cx));
                let h = p.height;
                *cache.borrow_mut() = Some(p);
                phys(cb, h)
            })
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
        // Отдельное дерево сохраняет точное начало хоста. Коробки и текст
        // округляются от общего абсолютного места, без переноса дробной
        // части начала в padding или позиции детей.
        let (vert, contain_floats) = (self.vert, self.contain_floats);
        let unr = window.layout_size_unrounded(*state);
        let cb = f32::from(if vert.is_some() {
            unr.height
        } else {
            unr.width
        });
        let origin = window.layout_origin_unrounded(*state);
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
                with_vert(vert, || {
                    window.with_nested_layout(|window| plan(&kids, cb, contain_floats, window, cx))
                })
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
        // Физический размер хоста: строчный `cb` и блочный `p.height`.
        let (pw, ph) = if vert.is_some() {
            (p.height, cb)
        } else {
            (cb, p.height)
        };
        let mut host = div().relative().w(px(pw)).h(px(ph));
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
            let el = with_vert(vert, || {
                frame(
                    kid.kind,
                    s.avail,
                    (kid.build)(cb, s.avail, s.shapes.clone(), s.h),
                    tap,
                )
            });
            // Логическое место → физическое (Blink
            // `writing_mode_converter.cc:80-102`): строчный сдвиг — вниз,
            // блочный — от правого края у `vertical-rl`, от левого у `-lr`.
            let (left, top) = match vert {
                None => (s.x, s.y),
                Some(true) => (p.height - s.y - s.b, s.x),
                Some(false) => (s.y, s.x),
            };
            host = host.child(
                div()
                    .absolute()
                    .left(px(left))
                    .top(px(top))
                    .child(el),
            );
        }
        let mut el = host.into_any_element();
        el.layout_as_root_at(
            origin,
            size(
                AvailableSpace::Definite(px(pw)),
                AvailableSpace::Definite(px(ph)),
            ),
            window,
            cx,
        );
        el.prepaint_at(point(px(0.0), px(0.0)), window, cx);
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
