//! Единственное межкадровое состояние пробы: принятые свойства выбранного варианта.

use crate::style::computed::{Align, CalcSizeBasis, Computed, PositionAnchor, Sides};
use crate::style::values::position_area::PositionArea;
use crate::style::values::value::Len;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Accepted {
    sizes: [Option<Len>; 6],
    inset: Sides,
    margin: Sides,
    align: [Option<Align>; 2],
    safe: [bool; 2],
    normal: [bool; 2],
    anchor: Option<PositionAnchor>,
    area: Option<PositionArea>,
    calc: [Option<(f32, f32, f32, f32)>; 4],
    calc_max: [Option<(f32, f32, f32, f32)>; 2],
    basis: [Option<CalcSizeBasis>; 6],
    stretch: [bool; 6],
    fit: [Option<Len>; 3],
    own_axis: [bool; 2],
    last: [bool; 2],
    flex_kw: bool,
    physical: Option<bool>,
    align_decl: Option<Option<Align>>,
    size_inherit: [bool; 6],
    margin_inherit: [bool; 4],
    inset_inherit: [bool; 4],
    align_inherit: (bool, u8),
}

impl Accepted {
    pub(super) fn of(c: &Computed) -> Self {
        Self {
            sizes: [
                c.width,
                c.height,
                c.min_width,
                c.min_height,
                c.max_width,
                c.max_height,
            ],
            inset: c.inset,
            margin: c.margin,
            align: [c.align_self, c.justify_self],
            safe: [c.align_self_safe, c.justify_self_safe],
            normal: [c.align_self_normal, c.justify_self_normal],
            anchor: c.position_anchor.clone(),
            area: c.position_area,
            calc: c.calc_size,
            calc_max: c.calc_size_max,
            basis: c.calc_size_basis,
            stretch: c.stretch_size,
            fit: c.fit_arg,
            own_axis: [c.align_self_own_axis, c.justify_self_own_axis],
            last: [c.align_self_last, c.justify_self_last],
            flex_kw: c.align_self_flex_kw,
            physical: c.justify_self_physical,
            align_decl: c.align_self_decl,
            size_inherit: [
                c.width_inherit,
                c.height_inherit,
                c.minmax_inherit[0],
                c.minmax_inherit[1],
                c.minmax_inherit[2],
                c.minmax_inherit[3],
            ],
            margin_inherit: c.margin_inherit,
            inset_inherit: c.inset_inherit,
            align_inherit: (
                c.align_self_inherit,
                c.align_inherit & crate::style::computed::ainh::JUSTIFY_SELF,
            ),
        }
    }

    pub(super) fn lengths(&mut self, mut f: impl FnMut(&mut Option<Len>)) {
        for value in &mut self.sizes {
            f(value);
        }
        for sides in [&mut self.inset, &mut self.margin] {
            for value in [
                &mut sides.top,
                &mut sides.right,
                &mut sides.bottom,
                &mut sides.left,
            ] {
                f(value);
            }
        }
        for value in &mut self.fit {
            f(value);
        }
    }

    pub(super) fn apply(&self, c: &mut Computed) {
        [
            c.width,
            c.height,
            c.min_width,
            c.min_height,
            c.max_width,
            c.max_height,
        ] = self.sizes;
        c.inset = self.inset;
        c.margin = self.margin;
        [c.align_self, c.justify_self] = self.align;
        [c.align_self_safe, c.justify_self_safe] = self.safe;
        [c.align_self_normal, c.justify_self_normal] = self.normal;
        c.position_anchor = self.anchor.clone();
        c.position_area = self.area;
        c.calc_size = self.calc;
        c.calc_size_max = self.calc_max;
        c.calc_size_basis = self.basis;
        c.stretch_size = self.stretch;
        c.fit_arg = self.fit;
        [c.align_self_own_axis, c.justify_self_own_axis] = self.own_axis;
        [c.align_self_last, c.justify_self_last] = self.last;
        c.align_self_flex_kw = self.flex_kw;
        c.justify_self_physical = self.physical;
        c.align_self_decl = self.align_decl;
        [c.width_inherit, c.height_inherit] = [self.size_inherit[0], self.size_inherit[1]];
        c.minmax_inherit.copy_from_slice(&self.size_inherit[2..]);
        c.margin_inherit = self.margin_inherit;
        c.inset_inherit = self.inset_inherit;
        c.align_self_inherit = self.align_inherit.0;
        let bit = crate::style::computed::ainh::JUSTIFY_SELF;
        c.align_inherit = (c.align_inherit & !bit) | self.align_inherit.1;
    }
}

/// Переносит только свойства fallback origin; остальные стили наследуются один раз.
pub(crate) fn apply_style(target: &mut Computed, computed: &Computed) {
    Accepted::of(computed).apply(target);
    // Значения уже вычислены до тактики: повторное inherit выбрало бы новую ось.
    target.width_inherit = false;
    target.height_inherit = false;
    target.minmax_inherit = [false; 4];
    target.margin_inherit = [false; 4];
    target.inset_inherit = [false; 4];
    target.align_self_inherit = false;
    target.align_inherit &= !crate::style::computed::ainh::JUSTIFY_SELF;
}
