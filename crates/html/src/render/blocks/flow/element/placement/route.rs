//! Выбор содержащего блока и слоя для позиционированной коробки.

use crate::layout::page::paged::PAGED;
use crate::layout::positioned::predicates::{edge_set, stays_positioned};
use crate::paint::stacking::{fixed_cb_layer_box, stacking_context};

use crate::style::computed::Computed;

#[allow(clippy::too_many_arguments)]
pub(crate) fn layer_route(
    e: &crate::dom::Element,
    inherited: &Computed,
    under_tf: bool,
    ordered_context: bool,
    geometry_layer_ok: bool,
    nodes: &[crate::dom::Node],
    idx: usize,
) -> (bool, bool, bool, bool, bool, bool, bool) {
    let x_set = edge_set(e.style.inset.left) || edge_set(e.style.inset.right);
    let y_set = edge_set(e.style.inset.top) || edge_set(e.style.inset.bottom);
    // `fixed` считается ОТ ОКНА всегда (§10.1 п.3): позиционированный
    // предок ему не содержащий блок, и заданной оси от него не
    // требуется — незаданная сторона держит статическое место. Пока он
    // шёл общим путём, коробка висела от края родителя.
    let fixed = e.style.position == Some(crate::style::computed::Position::Fixed) && !under_tf;
    // `fixed` под трансформом — абсолют относительно этого предка.
    let abs_like = e.style.position == Some(crate::style::computed::Position::Absolute)
        || (e.style.position == Some(crate::style::computed::Position::Fixed) && under_tf);
    // В стопке страниц абсолют корня уходит в слой и без заданных
    // сторон: на месте его резала бы маска фрагмента кида
    // (`monolithic-overflow-013`); статическую позицию копии 0 даёт
    // щуп, копии ≥ 1 идут непрерывным потоком от верха листа.
    let orphan_abs = abs_like
        && !(inherited.cb_ancestor || crate::text::inline::establishes_cb(inherited))
        && (x_set || y_set || PAGED.with(|p| p.get()));
    // Позиционированный предок ЕСТЬ, но это не родитель: коробку
    // забирает слой ближайшего содержащего блока (§10.1).
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (03.09, шесть заходов): пускать в слой
    // РОДИТЕЛЯ коробку, у которой родитель сам образует содержащий
    // блок (снять вето `!establishes_cb`). Замысел верный —
    // `Spot::fixed_axes` писался под смешанный случай «одна ось от
    // края, другая статическая», и без выноса такой коробке
    // статическую позицию не считает никто (`probe/svpc.html`: y = 30
    // вместо 90). Целевой срез 650 пар (305 зелёных): 258 при ЛЮБОМ
    // гейте — по позиции родителя, по флагу «слой открыт», без
    // табличных видов, только для одной оси. Приобретено 9, и это
    // ровно те пары, которые ждал прежний откат: `abspos-009`,
    // `position-absolute-007`, `right-offset-003`, `abs-pos-non-
    // replaced-vlr-087/089`, `-vrl-086/088/158/164`. Потеряно 46 —
    // `table-anonymous-objects-011..091`: родитель там обычный
    // `position: relative` div (`display: None`, слой открыт), и
    // коробка с ОДНОЙ заданной осью в его слое встаёт не туда, где
    // стояла в потоке. Значит неверно не условие входа, а сама
    // статическая позиция, которую слой считает горизонтальной
    // одноосной коробке. Возвращать вместе с проверкой щупа на
    // `table-anonymous-objects-011` (три абсолюта, у одного задан
    // лишь `top`).
    // `fixed` под трансформом: содержащий блок — ближайший предок,
    // содержащий `fixed` (css-transforms-1 §transform-rendering,
    // css-contain-2 §3.2), а позиционированные между ними — нет
    // (`out-of-flow-in-multicolumn-029`: `fixed` внутри абсолюта
    // внутри трансформа). Родитель-трансформ держит его на месте.
    let tf_fixed = e.style.position == Some(crate::style::computed::Position::Fixed) && under_tf;
    // Без заданных сторон — тоже: на месте раскладка разрешила бы
    // проценты размеров от РОДИТЕЛЯ (`width: 100%` у абсолютного
    // родителя нулевой ширины, `out-of-flow-in-multicolumn-044`), а
    // статическую позицию по обеим осям даёт щуп.
    let far_fixed = tf_fixed && !fixed_cb_layer_box(inherited);
    let far_abs = !tf_fixed
        && abs_like
        && inherited.cb_ancestor
        && !crate::text::inline::establishes_cb(inherited)
        && (x_set || y_set);
    let far_abs = far_abs || far_fixed;
    // A negative `z-index` box whose containing block is the ICB goes
    // to the ICB layer too, painted in the bottom layer (`Underlay`,
    // CSS 2.1 §9.9 step 3): in place it was positioned from its
    // parent's box, e.g. a `body` lowered by a collapsed margin
    // (spec-examples `shape-outside-001`: `#failure-container`).
    let below_icb = orphan_abs
        && !fixed
        && e.style.z_index.is_some_and(|z| z < 0)
        && !stacking_context(inherited);
    let to_icb = !ordered_context
        && geometry_layer_ok
        && (fixed || orphan_abs)
        && (e.style.z_index.unwrap_or(0) >= 0 || below_icb)
        && !stays_positioned(&nodes[idx + 1..]);
    // В гибком контейнере и сетке слой содержащего блока закрыт: там
    // нет щупа статической позиции. Но при ОБЕИХ заданных осях щуп и не
    // нужен (CSS 2.1 §10.1 п.4 — содержащий блок ближайший
    // позиционированный предок, а не flex/grid-родитель): иначе коробка
    // раскладывалась от родителя (`align-self-with-flex-grid-parent`:
    // розовый квадрат уезжал с `.inner` на 270 px).
    let to_cb = !to_icb
        && (!ordered_context || (x_set && y_set))
        && geometry_layer_ok
        && far_abs
        && e.style.z_index.unwrap_or(0) >= 0
        && !stays_positioned(&nodes[idx + 1..]);
    (x_set, y_set, fixed, below_icb, to_icb, to_cb, far_fixed)
}
