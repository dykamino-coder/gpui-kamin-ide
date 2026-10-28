//! Единственная подготовка геометрии копии для плана clone и StackChild.

use super::through_top::strip_through_top;
use crate::dom::Element;
use crate::layout::options::RenderOpts;

#[derive(Clone)]
pub(in crate::layout::multicol) struct Prepared {
    pub(super) copy: Element,
    pub(super) empty_floats: bool,
    pub(super) float_zero: bool,
}

/// Копия ребёнка стопки без собственных блочных полей (их кладёт стопка). Высоту,
/// переполнение и разрывы даёт раскладка копии (`lay_out_main`) до плана.
pub(in crate::layout::multicol) fn prepare(
    c: &Element,
    opts: &RenderOpts,
    col_vert: bool,
    col_rl: bool,
) -> Prepared {
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
    Prepared {
        copy,
        empty_floats,
        float_zero,
    }
}
