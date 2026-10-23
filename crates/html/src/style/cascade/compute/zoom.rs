//! Стадия вычисленных значений, `zoom` (css-viewport-1 §zoom-property): длины
//! каскадного стиля домножаются на действующий зум элемента (произведение
//! `zoom` по цепочке предков) в момент вычисления, как Blink
//! `CSSToLengthConversionData` с множителем `Zoom()`; арифметика — в
//! `style::zoom`. Значения от родителя (наследование и каскадное `inherit`)
//! несут зум родителя и домножаются на собственный множитель.
//!
//! Стадия идёт только у каскадного стиля (`Computed::zoom_pending`): стиль,
//! который построила раскладка, уже несёт используемые точки и получает лишь
//! действующий зум родителя.

use crate::style::computed::Computed;

/// Действующий зум, с которым родитель передаёт значения детям: содержимое
/// `<foreignObject>` раскладывается в единицах SVG и масштабируется вместе с
/// ним, поэтому начинает без зума.
pub(super) fn parent_eff(parent: &Computed) -> f32 {
    if parent.zoom_boundary {
        1.0
    } else {
        parent.zoom_eff.unwrap_or(1.0)
    }
}

/// Собственный множитель и действующий зум каскадного стиля `own`, если зум
/// что-то меняет.
pub(super) fn factors(parent: &Computed, own: &Computed) -> Option<(f32, f32)> {
    let k = own.zoom.unwrap_or(1.0);
    let eff = parent_eff(parent) * k;
    let changes = (eff - 1.0).abs() > f32::EPSILON || (k - 1.0).abs() > f32::EPSILON;
    changes.then_some((k, eff))
}

/// Длины каскадного стиля `own` в зуме элемента (`eff`, свой множитель `k`).
pub(super) fn zoom_own(
    parent: &Computed,
    own: &mut Computed,
    k: f32,
    eff: f32,
    vcx: crate::document::state::ValueCx<'_>,
) {
    own.zoom_eff = ((eff - 1.0).abs() > f32::EPSILON).then_some(eff);
    crate::style::zoom::apply(own, Some(parent), eff, k, vcx);
}

/// Родитель в зуме элемента с собственным множителем `k`: значения, которые
/// берёт каскадное `inherit` (css-cascade-5 §7.3.1: вычисленное значение
/// родителя, а длины элемента — в его зуме).
pub(super) fn parent_in_own_zoom(
    parent: &Computed,
    k: f32,
    vcx: crate::document::state::ValueCx<'_>,
) -> Option<Computed> {
    if (k - 1.0).abs() <= f32::EPSILON {
        return None;
    }
    let mut p = parent.clone();
    crate::style::zoom::scale_own(&mut p, k, vcx);
    Some(p)
}
