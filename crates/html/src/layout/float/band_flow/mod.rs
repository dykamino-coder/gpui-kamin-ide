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

use crate::layout::float::bands::FloatBands;
mod clearance;
mod kid;
mod piece;
use crate::layout::float::shapes::FloatShape;
use gpui::{AnyElement, App, IntoElement, LayoutId, Window, div, prelude::*, px};
pub use kid::{Kid, Kind, Nest};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
mod place;
use place::place_seq;
mod float_kid;
mod flow_kid;
use float_kid::{place_float, place_nest};
use flow_kid::place_flow;
mod element;
pub use element::BandFlow;
mod tap;
use tap::Tap;
mod probe;
use probe::{intrinsic, intrinsic_of, probe, probe_of};

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

/// План раскладки при ширине контекста `cb`: позиции всех детей и высота.
fn plan(kids: &[Kid], cb: f32, contain_floats: bool, window: &mut Window, cx: &mut App) -> Plan {
    let mut bands = FloatBands::new(cb);
    let (slots, y) = place_seq(kids, 0.0, cb, 0.0, true, (0.0, cb), &mut bands, window, cx);
    Plan {
        width: cb,
        // §§10.6.3, 10.6.7: only a formatting-context root contains floats.
        height: if contain_floats {
            bands.bottom(None).max(y)
        } else {
            y
        },
        slots,
    }
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
