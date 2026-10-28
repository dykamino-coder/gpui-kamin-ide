//! Сборка StackChild ребёнка стопки колонок: вложенный ряд, монолитность, число копий.

use super::stack_child_of;
use crate::dom::Element;
use crate::layout::cx::FrameCx;
use crate::layout::fragment::line_shape::nested_rows_box;
use crate::layout::fragment::probe::size_monolith;
use crate::style::computed::Display;
use crate::style::values::value::Len;

mod monolith;
use monolith::copy_monolith;

#[allow(clippy::too_many_arguments)]
pub(super) fn finish_stack_child(
    ix: usize,
    mt: f32,
    mb: f32,
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
    // Число копий — по измеренной протяжённости (`reserve_overflow_copies`).
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

/// Вложенный многоколоночник, которого режут ряды внешних колонок
/// (css-multicol-1 §8, css-break-3 §2: вложенный контекст фрагментации): его ряд
/// получает остаток внешнего фрагментаинера от места, где ребёнок начался
/// (`StackChild::nest`), а это место знает только раскладка предшествующих
/// детей (`ColumnStack::resume_nested`).
pub(in crate::layout::multicol) fn nest_candidate(
    ix: usize,
    kid_par: &[crate::layout::fragment::types::Par],
    col_vert: bool,
    copy: &Element,
) -> bool {
    !col_vert && kid_par.get(ix).is_none_or(|p| p.group == 0) && nested_rows_box(copy)
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
                // Атомарная строчная коробка стоит в строке, а строка
                // неделима (css-break-3 §4.1, §4.3 class B — только между
                // строками; Blink `LayoutBox::IsMonolithic`: `IsInline()`).
                || matches!(
                    copy.style.used().display,
                    Some(
                        Display::InlineBlock
                            | Display::InlineTable
                            | Display::InlineFlex
                            | Display::InlineGrid
                    )
                )
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
