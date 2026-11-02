//! Элемент GPUI `BandFlow` — хост полос обтекания с ИЗМЕРЕННЫМИ размерами: план размещения
//! детей (`Kid`, `Kind`, `Nest`) по полосам `FloatBands` строится пробной раскладкой, а не
//! литералами стиля. Вход: построители детей и их поля; выход: позиции и высота хоста.
//! Размеры детей берутся из раскладки:
//! * ширина контекста — из замера (`known`/`available`) и из размера узла в `prepaint`, а не
//!   из `style.width` (`<td>` без ширины в стиле, тело без ширины);
//! * размер флоата — shrink-to-fit CSS 2.1 §10.3.5 пробной раскладкой ДО посадки, как у
//!   Blink (`floats_utils.cc`, `LayoutFloatWithoutFragmentation`) и Servo
//!   (`flow/float.rs`, `FloatBox::layout` → `place_float_fragment`);
//! * кусок хвоста со своим контекстом без размеров — ширина окна полос и высота из
//!   содержимого (§9.5 последний абзац): перебор возможностей сверху вниз, как
//!   `BlockLayoutAlgorithm::LayoutNewFormattingContext` (Blink `block_layout_algorithm.cc`)
//!   и `layout_in_flow_block_level_sequentially` (Servo `flow/mod.rs`).
//!
//! Измеренный ребёнок сохраняет независимый движок до paint; держатель связывает его
//! с основным деревом через размер и окончательное абсолютное начало.

use crate::layout::float::bands::FloatBands;
mod clamp;
mod clearance;
mod kid;
mod piece;
mod prepaint;
use crate::layout::float::shapes::FloatShape;
use gpui::{AnyElement, App, IntoElement, LayoutId, Window, div, prelude::*, px};
pub use kid::{GroupWrap, Kid, Kind, Nest, ShapeOf};
use std::{cell::Cell, rc::Rc, sync::Arc};
mod place;
use place::place_seq;
mod float_kid;
mod flow_kid;
use float_kid::{place_float, place_nest};
use flow_kid::place_flow;
mod element;
mod fragment;
mod holders;
pub use element::BandFlow;
mod tap;
use tap::Tap;
mod cache;
mod prepared;
mod probe;
mod temporary;
#[cfg(test)]
mod tests;
use probe::{intrinsic, intrinsic_of, probe, probe_of};

/// Вырезы обтекания для строк потокового ребёнка: левые и правые формы от
/// его верха (`FloatBands::shapes`).
pub type Shapes = Arc<(Vec<FloatShape>, Vec<FloatShape>)>;

/// Построитель ребёнка: `(ширина контекста, доступная ширина, вырезы,
/// высота содержимого) -> элемент`. Готовая проба живёт до paint.
/// Высоту получает только `Kind::Nest`: его коробка
/// строится БЕЗ детей и высотой из плана.
pub type Build = Rc<
    dyn Fn(
        f32,
        f32,
        Option<Shapes>,
        Option<f32>,
        Option<crate::text::vertical::InlineMeasure>,
    ) -> AnyElement,
>;

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

/// Место ребёнка в плане: начало border-box, доступная ширина и готовая
/// проба либо размеры ещё не построенной оболочки вложенного блока.
#[derive(Clone, Debug, Default)]
struct Slot {
    prepared: prepared::Prepared,
    clamp: crate::text::clamp::MeasuredClamp,
    geometry: crate::layout::fragment::flow::FlowGeometry,
    x: f32,
    y: f32,
    avail: f32,
    shapes: Option<Shapes>,
    /// Resolved block margins of a float, preserved by fragmentation.
    float_margins: [f32; 2],
    /// Высота содержимого `Kind::Nest`.
    h: Option<f32>,
    /// Места детей `Kind::Nest`.
    kids: Vec<Slot>,
    /// Блочный размер border-box: в вертикальном письме по нему ставится
    /// физический левый край (`vertical-rl` идёт от ПРАВОГО края).
    b: f32,
    /// Measured border-box inline extent, for a bottom-to-top line axis.
    i: f32,
}

/// Пробы одного плана в логических осях (Blink `BfcOffset`): `vert` меняет
/// физическую ширину и высоту местами. Каркасы проб и держатели физические;
/// преобразование координат происходит в `holders`.
pub(super) struct Probe<'a> {
    engine: Option<prepared::Engine>,
    last_prepared: prepared::Prepared,
    paint: Rc<crate::layout::cx::deferred::Deferred>,
    build: crate::layout::build_contracts::ChildBuild,
    pub(super) window: &'a mut Window,
    pub(super) cx: &'a mut App,
    pub(super) vert: bool,
    fragment: Option<fragment::FragmentFlow>,
    clamp: Option<(u64, crate::text::clamp::ClampMeasure)>,
    last_clamp: crate::text::clamp::MeasuredClamp,
    positioning: Option<Rc<crate::layout::positioned::containing_block::Positioning>>,
    last_geometry: crate::layout::fragment::flow::FlowGeometry,
}

#[derive(Clone, Debug, Default)]
struct Plan {
    width: f32,
    height: f32,
    slots: Vec<Slot>,
}

pub(super) use crate::tolerance::COORD_EPS;

/// Отображение ребёнка в пробе: тот же самый каркас, что и в `prepaint`,
/// иначе проба и итог разойдутся.
///
/// Флоат — в колонке с `align-items: flex-start`: поперечный размер такого
/// элемента — fit-content = `min(max-content, max(min-content, доступно))`,
/// то есть ровно shrink-to-fit §10.3.5. Кусок — в колонке с растяжением:
/// `width: auto` у блока со своим контекстом заполняет окно (§10.3.3).
fn frame(
    kind: Kind,
    avail: f32,
    el: AnyElement,
    tap: Rc<Cell<Option<LayoutId>>>,
    vert: bool,
    paint: &Rc<crate::layout::cx::deferred::Deferred>,
    build: &crate::layout::build_contracts::ChildBuild,
) -> AnyElement {
    // Вертикальное письмо: строчная ось — физическая высота, каркас — ряд
    // высотой в окно, поперечная ось ряда и есть строчная (css-writing-modes-4
    // §7.1: shrink-to-fit §10.3.5 считается по строчной оси).
    let col = if vert {
        div().flex().flex_row().h(px(avail.max(0.0)))
    } else {
        div().flex().flex_col().w(px(avail.max(0.0)))
    };
    let mut col = if matches!(kind, Kind::Float { .. }) {
        col.items_start()
    } else {
        col
    };
    // CSS 2.1 Appendix E.2 step 5: a float is painted atomically, as if it
    // created a stacking context, whatever box it is (scroller, table,
    // replaced); its positioned descendants still belong to the enclosing
    // context, which the atomic paint passes them to.
    if matches!(kind, Kind::Float { .. }) {
        col = crate::hooks::paint_atomic(col);
    }
    let el = if matches!(kind, Kind::Float { .. }) {
        build.effects(
            el,
            crate::layout::build_contracts::Effect::OutlineScope(paint),
        )
    } else {
        el
    };
    col.child(crate::hooks::IntoHooked::into_hooked(Tap {
        inner: el,
        id: tap,
    }))
    .into_any_element()
}

/// План раскладки при ширине контекста `cb`: позиции всех детей и высота.
fn plan(kids: &[Kid], cb: f32, contain_floats: bool, p: &mut Probe) -> Plan {
    crate::work::add!(FloatPlans, 1);
    let mut bands = FloatBands::new(cb);
    let (slots, y) = place_seq(kids, 0.0, cb, 0.0, true, (0.0, cb), &mut bands, p);
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

/// Painting layer of a host child (CSS 2.1 Appendix E): in-flow block-level
/// boxes paint their backgrounds (step 4) below floats (step 5), and inline
/// content (step 7) above them. An anonymous inline run has no box of its
/// own, so it is painted entirely in the inline layer. A paint group
/// (`Nest::group`) is painted atomically with all its descendants on the
/// level of positioned boxes (step 8; CSS Color 4 §opacity).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Layer {
    Block,
    Float,
    Inline,
    Group,
}

fn layer(kid: &Kid) -> Layer {
    match kid.kind {
        Kind::Float { .. } => Layer::Float,
        _ if kid.anon => Layer::Inline,
        _ if kid.nest.as_ref().is_some_and(|n| n.group.is_some()) => Layer::Group,
        _ => Layer::Block,
    }
}

/// Дети слоя `layer` и их места плоским списком в порядке краски, на любой
/// глубине (коробка `Kind::Nest` раньше своих детей). The descendants of a
/// paint group belong to the group's own holder, not to the host's layers
/// (Blink `BoxFragmentPainter::PaintFloatingChildren` skips self-painting
/// layers).
fn flatten<'a>(
    kids: &'a [Kid],
    slots: &'a [Slot],
    which: Layer,
    out: &mut Vec<(&'a Kid, &'a Slot)>,
) {
    for (kid, s) in kids.iter().zip(slots.iter()) {
        let own = layer(kid);
        if own == which {
            out.push((kid, s));
        }
        if own == Layer::Group {
            continue;
        }
        if let Some(nest) = kid.nest.as_ref() {
            flatten(&nest.kids, &s.kids, which, out);
        }
    }
}
