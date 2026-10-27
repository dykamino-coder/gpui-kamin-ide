//! Единственная подготовка геометрии копии для плана clone и StackChild.

use super::cuts::copy_cuts_and_height;
use crate::dom::Element;
use crate::layout::fragment::Shape;
use crate::layout::fragment::shape_contents::strip_through_top;
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;

pub(in crate::layout::multicol) struct Prepared {
    pub(super) copy: Element,
    pub(super) empty_floats: bool,
    pub(super) float_zero: bool,
    pub(in crate::layout::multicol) cuts: Vec<(f32, f32)>,
    pub(in crate::layout::multicol) forced: Vec<f32>,
    pub(in crate::layout::multicol) solid: Vec<(f32, f32)>,
    pub(in crate::layout::multicol) h: f32,
    pub(in crate::layout::multicol) over: f32,
    pub(super) no_reach: bool,
}

#[allow(clippy::too_many_arguments)]
pub(in crate::layout::multicol) fn prepare(
    ix: usize,
    c: &Element,
    shape: &Shape,
    merged: &BoxStyle,
    opts: &RenderOpts,
    kid_par: &[crate::layout::fragment::types::Par],
    col_vert: bool,
    col_rl: bool,
    line_col_w: Option<f32>,
    fixed: Option<f32>,
    measured_kids: &std::cell::RefCell<Vec<(u64, f32)>>,
) -> Prepared {
    let Shape {
        height: h,
        cuts,
        forced,
        solid,
        ..
    } = shape.clone();
    let mut copy = c.clone();
    // Поля кладёт укладка колонок, не коробка. В
    // вертикальном письме блочные поля — левое и
    // правое; схлопывание сквозь верх (`strip_through_top`)
    // там не считается вовсе (мера повёрнутая).
    if col_vert {
        copy.style.used_mut().margin.left = None;
        copy.style.used_mut().margin.right = None;
    } else {
        copy.style.used_mut().margin.top = None;
        copy.style.used_mut().margin.bottom = None;
        // И поле, схлопнутое СКВОЗЬ верх (`through` в
        // `mt`): стопка уже положила его `lead`-ом,
        // второй раз его вставил бы корень копии.
        strip_through_top(&mut copy, 4);
    }
    let (empty_floats, float_zero) =
        super::floats::normalize(&mut copy, col_vert, col_rl, opts.cx.vcx());
    // TRANSITIONAL(F2-4): Clone/nested/OOF и ортогональные копии используют старую меру при общем отказе laid; удалить в P21 после per-child resume поверх P05/P08/P10.
    let laid = measured_kids
        .borrow()
        .iter()
        .any(|&(id, _)| id == copy.node_id)
        && (if col_vert {
            crate::layout::fragment::flow::vertical_copy(&copy, col_rl)
        } else {
            crate::layout::fragment::flow::independent_copy(&copy)
                || crate::layout::fragment::line_shape::nested_rows_box(&copy)
                || empty_floats
                || (copy.marks.paginate
                    && crate::layout::multicol::measure::native_float::float_copy(
                        &copy,
                        opts.cx.vcx(),
                    ))
        });
    let (cuts, forced, solid, h, over, no_reach) = if laid {
        // Высоту, переполнение и разрывы заменяет lay_out_main до плана.
        (cuts, forced, solid, h, h, true)
    } else {
        copy_cuts_and_height(
            ix, h, cuts, forced, solid, opts, kid_par, col_vert, col_rl, line_col_w, fixed, merged,
            &copy,
        )
    };
    Prepared {
        copy,
        empty_floats,
        float_zero,
        cuts,
        forced,
        solid,
        h,
        over,
        no_reach,
    }
}
