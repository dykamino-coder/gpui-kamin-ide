//! Единственное межкадровое состояние пробы: принятые свойства выбранного варианта.

use crate::style::computed::{Align, CalcSizeBasis, Computed, PositionAnchor, Sides};
use crate::style::css::LonghandId as L;
use crate::style::values::position_area::PositionArea;
use crate::style::values::value::Len;

const INHERITED: [L; 15] = [
    L::Width,
    L::Height,
    L::MinWidth,
    L::MinHeight,
    L::MaxWidth,
    L::MaxHeight,
    L::MarginTop,
    L::MarginRight,
    L::MarginBottom,
    L::MarginLeft,
    L::Top,
    L::Right,
    L::Bottom,
    L::Left,
    L::JustifySelf,
];

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
    inherit: [bool; 15],
    align_self_inherit: bool,
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
            inherit: INHERITED.map(|id| c.inherit_longhands.contains(id)),
            align_self_inherit: c.align_self_inherit,
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
        for (id, on) in INHERITED.into_iter().zip(self.inherit) {
            c.inherit_longhands.set(id, on);
        }
        c.align_self_inherit = self.align_self_inherit;
    }
}

/// Переносит только свойства fallback origin; остальные стили наследуются один раз.
pub(crate) fn apply_style(target: &mut Computed, computed: &Computed) {
    Accepted::of(computed).apply(target);
    // Значения уже вычислены до тактики: повторное inherit выбрало бы новую ось.
    for id in INHERITED {
        target.inherit_longhands.set(id, false);
    }
    target.align_self_inherit = false;
}
