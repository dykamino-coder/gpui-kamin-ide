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
    pick(&mut style.margin);
    pick(&mut style.padding);
    pick(&mut style.border_width);
    style.border_visible[side] = Some(false);
}
