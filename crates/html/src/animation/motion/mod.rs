//! ★ ДОЛГ (08.09, v158, замер второго прохода `settle`): полный свод +45 в
//! `css/motion`, но три пары ушли в минус и по бисекту (v163: в дереве только
//! этот патч) принадлежат ему: `anchor-position-inline-005` 0.05 → 0.67,
//! `-006` 0.08 → 1.00, `disclosure-styles` 0.19 → 0.62. Ни у одной нет
//! `offset-*` — задет либо снятый вызов из разбора стиля (`dom.rs: walk`),
//! либо порядок `settle` после `anchor::settle_static`. Разобрать отдельно.
//! `offset-path`/`offset-distance`/`offset-rotate`/`offset-anchor`/
//! `offset-position` (motion-1).
//!
//! Offset-трансформ по §«Calculating The Offset Transform» — это сдвиг,
//! совмещающий ТОЧКУ ПРИВЯЗКИ коробки (`offset-anchor`) с ТОЧКОЙ ПУТИ
//! (`offset-path` + `offset-distance`), и поворот (`offset-rotate`).
//! Отдельного конвейера под него нет: строка `transform` синтезируется и
//! уходит в тот же разборщик (`Computed::apply_one`), а матрицы складываются
//! в порядке слоения — сначала offset, поверх него авторский `transform`.
//!
//! В записи `transform`, где точка отсчёта — `transform-origin` `O`, а точка
//! привязки — `A`, это ровно
//!
//! ```text
//! translate(P) translate(-O) rotate(angle) translate(O - A)
//! ```
//!
//! `P` — точка пути В СИСТЕМЕ САМОЙ КОРОБКИ. При `offset-anchor: auto`
//! (начальное) `A == O`, хвост нулевой и не пишется вовсе.
//!
//! Считается это ВТОРЫМ ПРОХОДОМ по собранному дереву (`settle`), а не на
//! разборе стиля: §offset-path говорит «In CSS contexts, the boxes being
//! referenced are from the element that establishes the containing block for
//! this element» — опорную коробку `<coord-box>`, длину `ray()` (`<ray-size>`)
//! и начало `at <position>` даёт СОДЕРЖАЩИЙ БЛОК, которого на разборе стиля
//! ещё нет. Где содержащий блок статически не выводится (доля или `auto` у
//! его ширины/высоты; начальный содержащий блок — его размер знает только
//! отрисовка), проход отдаёт `None`, и работают ровно те ветки, что работали
//! раньше: `path()` и `ray()` с пиксельным `offset-distance`.

mod path;
use path::flatten;
use path::length;
use path::sample;

mod curves;

mod ray;
use ray::angle_rad;
use ray::ray_css;

mod anchors;
use anchors::origin_shift;
use anchors::rotation;
use anchors::start_of;

mod transform;
use transform::offset_transform_css;

mod containing;
use containing::walk;

mod shapes;
use shapes::collect_shapes;

use crate::dom::Node;
use crate::style::computed::Computed;

mod ellipse_path;

/// Ломаная контура: точки в системе координат коробки и признак замыкания.
struct Poly {
    pts: Vec<(f32, f32)>,
    closed: bool,
}

/// Опорные коробки СОДЕРЖАЩЕГО БЛОКА в его собственной системе (начало —
/// левый верхний угол его border-коробки) плюс место коробки элемента в ней.
/// Всё в css-точках.
#[derive(Clone, Copy, Debug, Default)]
pub struct Cb {
    /// `<coord-box>` по номеру: 0 margin, 1 border, 2 padding, 3 content —
    /// каждая как (x, y, w, h).
    pub boxes: [(f32, f32, f32, f32); 4],
    /// Радиусы border-коробки содержащего блока (tl, tr, br, bl),
    /// эллиптические. Нужны голому `<coord-box>`: §offset-path «If
    /// `<offset-path>` is omitted, it defaults to `inset(0 round X)`, where X
    /// is the value of `border-radius` on the element that establishes the
    /// containing block for this element».
    pub radius: [(f32, f32); 4],
    /// Левый верхний угол border-коробки ЭЛЕМЕНТА в той же системе.
    pub self_off: (f32, f32),
    /// Размер border-коробки элемента — его требует ключ `contain`.
    pub self_size: (f32, f32),
}

/// Второй проход по дереву коробок: каждому элементу с `offset-path`
/// считается offset-трансформ по геометрии его СОДЕРЖАЩЕГО БЛОКА.
pub fn settle(nodes: &mut [Node]) {
    // `offset-path: url(#id)` ссылается на SVG-фигуру ЛЮБОГО места документа —
    // словарь эквивалентных путей собирается до обхода и живёт ровно один
    // проход (вложенный документ `<iframe>` зовёт `settle` со своим деревом).
    let mut shapes = std::collections::HashMap::new();
    collect_shapes(nodes, &mut shapes);
    SVG_SHAPES.with(|m| *m.borrow_mut() = shapes);
    walk(nodes, None);
    SVG_SHAPES.with(|m| m.borrow_mut().clear());
}

thread_local! {
    /// `id` → эквивалентный путь (SVG 2 §shapes, «equivalent path») фигур
    /// документа — для `offset-path: url(#id)`.
    static SVG_SHAPES: std::cell::RefCell<std::collections::HashMap<String, String>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Приставить offset-трансформ к авторскому.
pub fn apply_offset_transform(c: &mut Computed, cb: Option<&Cb>) {
    let Some(css) = offset_transform_css(c, cb) else {
        return;
    };
    let author = c.transform.take();
    c.apply_one("transform", &css);
    if let (Some(off), Some(a)) = (c.transform, author) {
        c.transform = Some(off.then(&a));
    }
}

/// Исходные параметры offset-трансформа для расчётов в дочерних модулях.
pub(super) struct MotionStyle<'a> {
    offset_path: &'a Option<String>,
    offset_distance: Option<crate::style::values::value::Len>,
    offset_rotate: &'a Option<String>,
    offset_anchor: &'a Option<String>,
    offset_position: &'a Option<String>,
}

pub(super) fn motion_style(c: &Computed) -> MotionStyle<'_> {
    MotionStyle {
        offset_path: &c.offset_path,
        offset_distance: c.offset_distance,
        offset_rotate: &c.offset_rotate,
        offset_anchor: &c.offset_anchor,
        offset_position: &c.offset_position,
    }
}
