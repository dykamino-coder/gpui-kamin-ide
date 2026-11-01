//! Единственное межкадровое состояние пробы: принятые свойства выбранного варианта.

use crate::style::computed::{Align, CalcSizeBasis, Computed, PositionAnchor, Sides};
use crate::style::css::LonghandId as L;
use crate::style::values::position_area::PositionArea;
use crate::style::values::value::Len;

const INHERITED: [L; 16] = [
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
    L::AlignSelf,
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
    inherit: [bool; 16],
}

impl Accepted {
    pub(super) fn of(c: &Computed) -> Self {
        Self {
            sizes: [
                c.reset_box_layout.width,
                c.reset_box_layout.height,
                c.reset_box_layout.min_width,
                c.reset_box_layout.min_height,
                c.reset_box_layout.max_width,
                c.reset_box_layout.max_height,
            ],
            inset: c.reset_positioned.inset,
            margin: c.reset_box_layout.margin,
            align: [c.reset_flex.align_self, c.reset_flex.justify_self],
            safe: [c.reset_flex.align_self_safe, c.reset_flex.justify_self_safe],
            normal: [
                c.reset_flex.align_self_normal,
                c.reset_flex.justify_self_normal,
            ],
            anchor: c.reset_positioned.position_anchor.clone(),
            area: c.reset_positioned.position_area,
            calc: c.reset_box_layout.calc_size,
            calc_max: c.reset_box_layout.calc_size_max,
            basis: c.reset_box_layout.calc_size_basis,
            stretch: c.reset_box_layout.stretch_size,
            fit: c.reset_context.fit_arg,
            own_axis: [
                c.reset_flex.align_self_own_axis,
                c.reset_flex.justify_self_own_axis,
            ],
            last: [c.reset_flex.align_self_last, c.reset_flex.justify_self_last],
            flex_kw: c.reset_flex.align_self_flex_kw,
            physical: c.reset_flex.justify_self_physical,
            align_decl: c.reset_flex.align_self_decl,
            inherit: INHERITED.map(|id| c.reset_cascade.inherit_longhands.contains(id)),
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
            c.reset_box_layout.width,
            c.reset_box_layout.height,
            c.reset_box_layout.min_width,
            c.reset_box_layout.min_height,
            c.reset_box_layout.max_width,
            c.reset_box_layout.max_height,
        ] = self.sizes;
        c.reset_positioned.inset = self.inset;
        c.reset_box_layout.margin = self.margin;
        [c.reset_flex.align_self, c.reset_flex.justify_self] = self.align;
        [c.reset_flex.align_self_safe, c.reset_flex.justify_self_safe] = self.safe;
        [
            c.reset_flex.align_self_normal,
            c.reset_flex.justify_self_normal,
        ] = self.normal;
        c.reset_positioned.position_anchor = self.anchor.clone();
        c.reset_positioned.position_area = self.area;
        c.reset_box_layout.calc_size = self.calc;
        c.reset_box_layout.calc_size_max = self.calc_max;
        c.reset_box_layout.calc_size_basis = self.basis;
        c.reset_box_layout.stretch_size = self.stretch;
        c.reset_context.fit_arg = self.fit;
        [
            c.reset_flex.align_self_own_axis,
            c.reset_flex.justify_self_own_axis,
        ] = self.own_axis;
        [c.reset_flex.align_self_last, c.reset_flex.justify_self_last] = self.last;
        c.reset_flex.align_self_flex_kw = self.flex_kw;
        c.reset_flex.justify_self_physical = self.physical;
        c.reset_flex.align_self_decl = self.align_decl;
        for (id, on) in INHERITED.into_iter().zip(self.inherit) {
            c.reset_cascade.inherit_longhands.set(id, on);
        }
    }
}

/// Переносит только свойства fallback origin; остальные стили наследуются один раз.
pub(crate) fn apply_style(target: &mut Computed, computed: &Computed) {
    Accepted::of(computed).apply(target);
    // Значения уже вычислены до тактики: повторное inherit выбрало бы новую ось.
    for id in INHERITED {
        target.reset_cascade.inherit_longhands.set(id, false);
    }
}
