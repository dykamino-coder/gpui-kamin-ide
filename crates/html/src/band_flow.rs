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
    InspectorElementId, IntoElement, LayoutId, Pixels, Window, div, prelude::*, px, size,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

/// Вырезы обтекания для строк потокового ребёнка: левые и правые формы от
/// его верха (`FloatBands::shapes`).
pub type Shapes = Arc<(Vec<FloatShape>, Vec<FloatShape>)>;

/// Построитель ребёнка: `(ширина контекста, доступная ширина, вырезы) ->
/// элемент`. Вызывается на каждую пробу заново (см. доккомент модуля).
pub type Build = Rc<dyn Fn(f32, f32, Option<Shapes>) -> AnyElement>;

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
}

/// Место ребёнка в плане: левый верх border-box, доступная ширина (по ней
/// ребёнок строится заново в `prepaint`).
#[derive(Clone, Debug, Default)]
struct Slot {
    x: f32,
    y: f32,
    avail: f32,
    shapes: Option<Shapes>,
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
    let mut el = frame(kid.kind, avail, (kid.build)(cb, avail, shapes), tap.clone());
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
        inner: (kid.build)(0.0, 0.0, None),
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
    let mut slots = vec![Slot::default(); kids.len()];
    // Потолок потока: низ предыдущего куска плюс его нижнее поле (правило 5
    // §9.5.1 для кусков: окно ищется не выше).
    let mut y = 0.0f32;
    for (k, kid) in kids.iter().enumerate() {
        let [mt, mr, mb, ml] = kid.margin.map(|e| e.at(cb));
        match kid.kind {
            Kind::Float {
                side,
                clear,
                shrink,
            } => {
                let mut avail = (cb - ml - mr).max(0.0);
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
                let (bw, bh) = probe(kid, cb, avail, None, window, cx);
                // Посадка margin-box: правила 1-9 §9.5.1 и clear §9.5.2 — в
                // `bands.add_float`.
                let (fx, fy) = bands.add_float(side, ml + bw + mr, mt + bh + mb, clear);
                slots[k] = Slot {
                    x: fx + ml,
                    y: fy + mt,
                    avail,
                    shapes: None,
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
                // §9.5, последний абзац: border-box куска не перекрывает
                // margin-box флоатов; поля работают только по блочной оси
                // (как в статическом хосте `shape_flow` и у Blink
                // `block_layout_algorithm.cc:2164-2172` — «Margins are
                // applied from the content-box, not the layout opportunity
                // area»). Перебор окон сверху вниз: ширина окна на верхней
                // полосе → проба → проверка окна на всю высоту пробы.
                // §9.5.2: гипотетическая позиция — `y + mt` (поля уже
                // схлопнуты на уровне узлов); не ниже флоатов — clearance
                // ставит верх рамки ровно на их низ (Blink `AdjustToClearance`,
                // `space_utils.cc:22-30`).
                let mut top = bands.clearance(kid.clear, y + mt);
                // Окно `[l, r)` → (левый край коробки, доступная ширина) по
                // Blink `block_layout_algorithm.cc:2136-2178`: окно, не
                // суженное флоатами, урезается полями; суженное — нет, поля
                // откладываются от края СОДЕРЖАЩЕГО БЛОКА («Margins are
                // applied from the content-box, not the layout opportunity
                // area»), и окно лишь сжимается, если поле длиннее флоата.
                let edge = |l: f32, r: f32| {
                    let (has_l, has_r) = (l > EPS, r < cb - EPS);
                    let (ll, rr) = if !has_l && !has_r {
                        (l + ml, r - mr)
                    } else {
                        (l.max(ml.max(0.0)), r.min(cb - mr.max(0.0)))
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
                let (x, t, avail) = loop {
                    let (l, r) = bands.available(top, 0.0);
                    let (x, avail, has_l, has_r) = edge(l, r);
                    let (bw, bh) = probe(kid, cb, avail, None, window, cx);
                    let bw = bw.max(floor);
                    // Окно на всю высоту пробы (`:2209`: блочный размер
                    // фрагмента не больше возможности).
                    let (l2, r2) = bands.available(top, bh);
                    if (l2 - l).abs() < EPS && (r2 - r).abs() < EPS {
                        if fits(l, r, x, bw, has_l, has_r) {
                            break (x, top, avail);
                        }
                    } else {
                        // Ниже по высоте окно уже — проба в нём ещё раз
                        // (Servo `try_to_expand_for_auto_block_size`,
                        // `flow/float.rs:260`).
                        let (x2, avail2, h_l, h_r) = edge(l2, r2);
                        let (bw2, bh2) = probe(kid, cb, avail2, None, window, cx);
                        let bw2 = bw2.max(floor);
                        let (l3, r3) = bands.available(top, bh2);
                        if l3 <= l2 + EPS && r3 >= r2 - EPS && fits(l2, r2, x2, bw2, h_l, h_r) {
                            break (x2, top, avail2);
                        }
                    }
                    match bands.next_edge(top) {
                        Some(t) => top = t,
                        // Ниже всех флоатов — на всю ширину.
                        None => break (ml, top, (cb - ml - mr).max(0.0)),
                    }
                };
                let (_, bh) = probe(kid, cb, avail, None, window, cx);
                slots[k] = Slot {
                    x,
                    y: t,
                    avail,
                    shapes: None,
                };
                y = t + bh + mb;
            }
            Kind::Flow => {
                // Коробка во всю ширину содержащего блока: флоаты её
                // перекрывают, а строки получают вырезы полос от её верха
                // (Servo `place_line_among_floats`, `inline/mod.rs:1466`;
                // Blink `ComputeLineLayoutOpportunity`,
                // `layout_opportunity.cc:164-189`).
                let top = bands.clearance(kid.clear, y + mt);
                let avail = (cb - ml - mr).max(0.0);
                let shapes = bands.shapes(top);
                let (_, bh) = probe(kid, cb, avail, Some(shapes.clone()), window, cx);
                slots[k] = Slot {
                    x: ml,
                    y: top,
                    avail,
                    shapes: Some(shapes),
                };
                y = top + bh + mb;
            }
        }
    }
    Plan {
        width: cb,
        // §10.6.7: хост охватывает флоаты — они абсолютные и высоту сами не
        // растят (как `min_h` статического хоста).
        height: bands.bottom(None).max(y),
        slots,
    }
}

/// Ширина контекста для внутреннего размера: max-content — все флоаты в
/// одну строку плюс самый широкий кусок, min-content — самый широкий из
/// всех (Blink `BlockNode::ComputeMinMaxSizes`: флоаты копят инлайн-размер
/// до `clear`, кусок прибавляется к ним).
fn intrinsic_width(kids: &[Kid], max: bool, window: &mut Window, cx: &mut App) -> f32 {
    let (mut floats, mut piece, mut widest) = (0.0f32, 0.0f32, 0.0f32);
    for kid in kids {
        if matches!(kid.kind, Kind::Strut(_)) {
            continue;
        }
        let [_, mr, _, ml] = kid.margin.map(|e| e.at(0.0));
        let (mn, mx) = intrinsic(kid, window, cx);
        widest = widest.max(mn + ml + mr);
        match kid.kind {
            Kind::Float { .. } => floats += mx + ml + mr,
            _ => piece = piece.max(mx + ml + mr),
        }
    }
    if max {
        (floats + piece).max(widest)
    } else {
        widest
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
        _cx: &mut App,
    ) -> (LayoutId, ()) {
        let kids = self.kids.clone();
        let cache = self.plan.clone();
        let mut style = gpui::Style::default();
        // Блочный ребёнок колонки потока: во всю ширину и без сжатия, как
        // `div().relative().w_full()` статического хоста.
        style.size.width = gpui::relative(1.).into();
        style.flex_shrink = 0.0;
        let id = window.request_measured_layout(style, move |known, available, window, cx| {
            let cb = match (known.width, available.width) {
                (Some(w), _) => f32::from(w),
                (None, AvailableSpace::Definite(w)) => f32::from(w),
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
        (id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let cb = f32::from(bounds.size.width);
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
        let mut host = div().relative().w(px(cb)).h(px(p.height));
        // CSS 2.1 прил. E: фоны блоков потока (шаг 4) — РАНЬШЕ флоатов
        // (шаг 5): флоат лежит поверх блока, под которым стоит
        // (`clear-004`). Строки рядом с флоатом его не перекрывают — их
        // порядок с флоатом не виден.
        let order = self
            .kids
            .iter()
            .zip(p.slots.iter())
            .filter(|(k, _)| !matches!(k.kind, Kind::Float { .. }))
            .chain(
                self.kids
                    .iter()
                    .zip(p.slots.iter())
                    .filter(|(k, _)| matches!(k.kind, Kind::Float { .. })),
            );
        for (kid, s) in order {
            if matches!(kid.kind, Kind::Strut(_)) {
                continue;
            }
            let tap = Rc::new(Cell::new(None));
            let el = frame(
                kid.kind,
                s.avail,
                (kid.build)(cb, s.avail, s.shapes.clone()),
                tap,
            );
            host = host.child(div().absolute().left(px(s.x)).top(px(s.y)).child(el));
        }
        let mut el = host.into_any_element();
        el.layout_as_root(
            size(
                AvailableSpace::Definite(px(cb)),
                AvailableSpace::Definite(px(p.height)),
            ),
            window,
            cx,
        );
        el.prepaint_at(bounds.origin, window, cx);
        self.built.push(el);
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
