//! Фрагменты охватчика: видимость, доля высоты, части многоколоночника.

use super::spanner_edge;
use crate::dom::{Element, Node};
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::fragment_size::shape_full;
use crate::render::is_blank;
use crate::style::values::value::Len;
mod parts;
pub(super) use parts::spanner_parts;

/// Виден ли фрагмент предка: непустое содержимое или кромка коробки на
/// своей стороне разреза. Пустой фрагмент с кромкой обязателен —
/// css-multicol-1 §column-span, `Overview.bs:1540-1541`: «If the fragment
/// before the spanner is empty, nothing special happens; the top
/// margin/border/padding is above the spanning element, as an empty
/// fragment».
pub(super) fn spanner_frag_visible(c: &Element, kids: &[Node], first: bool, last: bool) -> bool {
    if kids.iter().any(|n| !is_blank(n)) {
        return true;
    }
    let b = &c.style.border_width;
    (first
        && (spanner_edge(&c.style.margin.top)
            || spanner_edge(&c.style.padding.top)
            || spanner_edge(&b.top)))
        || (last
            && (spanner_edge(&c.style.margin.bottom)
                || spanner_edge(&c.style.padding.bottom)
                || spanner_edge(&b.bottom)))
}

/// Фрагмент предка спаннера. css-break-3 §4.3 (вид `slice`, умолчание
/// `box-decoration-break`): верхние поле/рамка/отбивка — только у первого
/// фрагмента, нижние — только у последнего.
///
/// `keep_size` — отдать фрагменту ЗАДАННУЮ блочную высоту предка. Она
/// принадлежит коробке ЦЕЛИКОМ и расходуется фрагментами по очереди (Blink
/// `fragmentation_utils.cc`: остаток блочного размера считается от уже
/// уложенных фрагментов). Разложить остаток по фрагментам на уровне дерева
/// нечем — геометрия колонок здесь ещё не известна, — поэтому высота
/// ставится ТОЛЬКО когда предок на деле не разошёлся: видимый фрагмент
/// один. Разошёлся на несколько — каждый меряется по содержимому, а не
/// повторяет `height` предка целиком (иначе `height: 200px` удвоилась бы:
/// `non-adjacent-spanners-001`).
pub(super) fn spanner_fragment(
    c: &Element,
    kids: Vec<Node>,
    first: bool,
    last: bool,
    keep_size: bool,
    ix: usize,
) -> Element {
    let mut f = c.clone();
    f.children = kids;
    if !first {
        f.style.margin.top = None;
        f.style.padding.top = None;
        f.style.border_width.top = None;
        // Устойчивый номер узла у продолжения свой: по нему GPUI хранит
        // состояние (анимация, буферы линеек промежутков), и два фрагмента
        // с одним номером слились бы в один.
        f.node_id = c.node_id ^ (ix as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
    if !last {
        f.style.margin.bottom = None;
        f.style.padding.bottom = None;
        f.style.border_width.bottom = None;
    }
    if !keep_size {
        f.style.height = None;
        f.style.min_height = None;
    }
    f
}

/// Доли заданной блочной высоты предка по его фрагментам вокруг спаннеров.
/// Blink (`fragmentation_utils.cc`, `FinishFragmentation`) расходует
/// `block-size` коробки фрагментами ПО ОЧЕРЕДИ: непоследний фрагмент берёт
/// своё содержимое, но не больше остатка, последний — весь остаток
/// (`multicol-span-all-children-height-004a`: 450 = 200 + 200 + 50, `-005`:
/// 250 = 100 + 100 + 50; `non-adjacent-spanners-000`, `parallel-flow-after-
/// spanner-002`: до спаннера содержимого нет — вся высота ПОСЛЕ него).
/// `None` — правило не берётся (высота не в точках, фрагмент один, виден
/// один и не после пустых, мера не вышла), и действует прежний `keep_size`.
/// В векторе: `Some(v)` — высота фрагмента, `None` — своя, по содержимому.
pub(super) fn spanner_height_share(c: &Element, bodies: &[Vec<Node>]) -> Option<Vec<Option<f32>>> {
    let Some(Len::Px(total)) = c.style.height else {
        return None;
    };
    let n = bodies.len();
    if n < 2 {
        return None;
    }
    let empty = |b: &Vec<Node>| b.iter().all(is_blank);
    let pre_empty = bodies[..n - 1].iter().all(&empty);
    let seen = bodies
        .iter()
        .enumerate()
        .filter(|(i, b)| spanner_frag_visible(c, b, *i == 0, *i + 1 == n))
        .count();
    if !pre_empty && seen <= 1 {
        return None;
    }
    let px = |l: &Option<Len>| match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    };
    let edge = px(&c.style.padding.top)? + px(&c.style.borders().top)?;
    let mut left = total;
    let mut out = Vec::with_capacity(n);
    for (i, b) in bodies.iter().enumerate() {
        if i + 1 == n {
            out.push(Some(left.max(0.0)));
            break;
        }
        if empty(b) {
            out.push(None);
            continue;
        }
        let f = spanner_fragment(c, b.clone(), i == 0, false, false, i);
        let h = shape_full(&f, 4, ShapeCx::COLUMNS)?.0;
        let own = (h - if i == 0 { edge } else { 0.0 }).max(0.0);
        let take = own.min(left);
        out.push((own > left + 0.01).then_some(take));
        left -= take;
    }
    Some(out)
}
