//! Сборка StackChild ребёнка стопки колонок: вложенный ряд, монолитность, число копий.

use super::stack_child_of;
use crate::dom::Element;
use crate::layout::cx::FrameCx;
use crate::layout::fragment::line_shape::nested_rows_box;
use crate::layout::fragment::probe::size_monolith;
use crate::style::values::value::Len;
use crate::tolerance::COORD_EPS;

mod monolith;
use monolith::copy_monolith;

#[allow(clippy::too_many_arguments)]
pub(super) fn finish_stack_child(
    ix: usize,
    mt: f32,
    mb: f32,
    e: &Element,
    kid_par: &[crate::layout::fragment::types::Par],
    col_vert: bool,
    rows: Option<crate::layout::fragment::types::Rows>,
    copies: usize,
    fixed: Option<f32>,
    measured_kids: &std::cell::RefCell<Vec<(u64, f32)>>,
    dec: Option<(f32, f32)>,
    copy: &Element,
    cuts: Vec<(f32, f32)>,
    forced: Vec<f32>,
    solid: Vec<(f32, f32)>,
    h: f32,
    over: f32,
    rel: (f32, f32),
    nest_row: Option<f32>,
    whole: bool,
    build: super::factory::Builder,
    measured_table: Option<(bool, bool)>,
    vcx: FrameCx<'_>,
) -> crate::layout::fragment::types::StackChild {
    let (scrolls, monolith) = copy_monolith(copy, &cuts, nest_row);
    // Высоту меряет раскладка копии (`StackChild::measure`):
    // точек разреза мера не дала, и строчный набор без них
    // не монолит — режется краем колонки. Монолит — только
    // по собственным причинам коробки (css-break-3 §4.1).
    let (measure, monolith) =
        measured_monolith(measured_kids, copy, nest_row, whole, scrolls, monolith);
    let span = copy.style.used().column_span == Some(true) && !copy.inline;
    // Число копий — по измеренной протяжённости (eserve_overflow_copies\).
    let kid_copies = copies;
    let mut child = stack_child_of(
        ix,
        mt,
        mb,
        kid_par,
        col_vert,
        rows,
        fixed,
        dec,
        copy,
        cuts,
        forced,
        solid,
        h,
        over,
        rel,
        build,
        measure,
        monolith,
        span,
        kid_copies,
        measured_table,
        vcx,
    );
    child.fit_whole = whole && fixed.is_some() && measure.is_some();
    child
}

#[allow(clippy::too_many_arguments)]
pub(super) fn nest_row_of(
    ix: usize,
    kid_par: &[crate::layout::fragment::types::Par],
    col_vert: bool,
    rows: Option<crate::layout::fragment::types::Rows>,
    fixed_nest: Option<f32>,
    balanced_frag: Option<f32>,
    nest_at: &[Option<f32>],
    nested_auto: &std::cell::RefCell<Vec<u64>>,
    copy: &Element,
) -> Option<f32> {
    fixed_nest.filter(|hh| {
        *hh > 0.0
            && (rows.is_none() || balanced_frag.is_some())
            && !col_vert
            && kid_par.get(ix).is_none_or(|p| p.group == 0)
            && nested_rows_box(copy)
            && match nest_at.get(ix).copied().flatten() {
                // С верха колонки — и заданная высота, и
                // `auto` с мерой рядами.
                Some(y0) if y0 < COORD_EPS => {
                    matches!(copy.style.used().height, Some(Len::Px(_)))
                        || nested_auto.borrow().contains(&copy.node_id)
                }
                // Ниже верха — только заданная высота: мера
                // коробки от рядов не зависит.
                Some(_) => matches!(copy.style.used().height, Some(Len::Px(_))),
                None => false,
            }
    })
}

pub(super) fn measured_monolith(
    measured_kids: &std::cell::RefCell<Vec<(u64, f32)>>,
    copy: &Element,
    nest_row: Option<f32>,
    whole: bool,
    scrolls: impl Fn(Option<crate::style::computed::Overflow>) -> bool,
    monolith: bool,
) -> (Option<f32>, bool) {
    let measure = measured_kids
        .borrow()
        .iter()
        .find(|(id, _)| *id == copy.node_id)
        .map(|(_, w)| *w);
    let monolith = if whole {
        true
    } else if measure.is_some() {
        nest_row.is_none()
            && (size_monolith(copy)
                || copy.style.used().break_inside_avoid
                // css-break-3 §4.1 допускает монолитный overflow:hidden
                // с auto block size; сохраняем выбор старого пути для строк.
                || (monolith
                    && matches!(copy.style.used().height, None | Some(Len::Auto))
                    && [copy.style.used().overflow_x, copy.style.used().overflow_y].contains(
                        &Some(crate::style::computed::Overflow::Hidden)))
                || scrolls(copy.style.used().overflow_x)
                || scrolls(copy.style.used().overflow_y))
    } else {
        monolith
    };
    (measure, monolith)
}
