//! Снятие физических боковых декораций у фрагментов разрезанного инлайна.

use crate::style::values::value::Len;

/// Remove the margin, border and padding of one physical inline side
/// (1 = right, 3 = left) of a fragment of a split inline box.
pub(super) fn drop_inline_side(style: &mut crate::style::computed::Computed, side: usize) {
    let zero = Some(crate::style::values::value::Len::Px(0.0));
    let pick = |s: &mut crate::style::computed::Sides| {
        if side == 1 {
            s.right = zero;
        } else {
            s.left = zero;
        }
    };
    pick(&mut style.reset_box_layout.margin);
    pick(&mut style.reset_box_layout.padding);
    pick(&mut style.reset_border.border_width);
    style.reset_border.border_visible[side] = Some(false);
}

/// Ненулевая рамка, отбивка или поле физического края инлайна.
pub(super) fn has_edge(st: &crate::style::computed::Computed, side: usize) -> bool {
    let geometry = &st.reset_box_layout;
    let nz = |l: Option<Len>| matches!(l, Some(Len::Px(v)) if v.abs() > 0.0);
    let (b, p, m) = if side == 1 {
        (
            st.borders().right,
            geometry.padding.right,
            geometry.margin.right,
        )
    } else {
        (
            st.borders().left,
            geometry.padding.left,
            geometry.margin.left,
        )
    };
    nz(b) || nz(p) || nz(m)
}
