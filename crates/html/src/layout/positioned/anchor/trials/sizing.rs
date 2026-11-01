//! Перестановка входов расчёта размеров при перпендикулярной try-тактике.

use crate::style::computed::Computed;
use crate::style::css::LonghandId as L;
use crate::style::values::value::{AnchorFn, AnchorSize, Len, anchor_get, anchor_store};
use crate::values::ValueCx;

pub(super) fn transpose(c: &mut Computed, vcx: ValueCx<'_>) {
    std::mem::swap(
        &mut c.reset_box_layout.width,
        &mut c.reset_box_layout.height,
    );
    std::mem::swap(
        &mut c.reset_box_layout.min_width,
        &mut c.reset_box_layout.min_height,
    );
    std::mem::swap(
        &mut c.reset_box_layout.max_width,
        &mut c.reset_box_layout.max_height,
    );
    for (a, b) in [
        (L::Width, L::Height),
        (L::MinWidth, L::MinHeight),
        (L::MaxWidth, L::MaxHeight),
    ] {
        let av = c.reset_cascade.inherit_longhands.contains(a);
        let bv = c.reset_cascade.inherit_longhands.contains(b);
        c.reset_cascade.inherit_longhands.set(a, bv);
        c.reset_cascade.inherit_longhands.set(b, av);
    }
    c.reset_box_layout.calc_size.swap(0, 1);
    c.reset_box_layout.calc_size.swap(2, 3);
    c.reset_box_layout.calc_size_max.swap(0, 1);
    c.reset_box_layout.calc_size_basis.swap(0, 1);
    c.reset_box_layout.calc_size_basis.swap(2, 3);
    c.reset_box_layout.calc_size_basis.swap(4, 5);
    c.reset_box_layout.stretch_size.swap(0, 1);
    c.reset_box_layout.stretch_size.swap(2, 4);
    c.reset_box_layout.stretch_size.swap(3, 5);
    for style_field_index in 0..6 {
        let l = match style_field_index {
            0 => &mut c.reset_box_layout.width,
            1 => &mut c.reset_box_layout.height,
            2 => &mut c.reset_box_layout.min_width,
            3 => &mut c.reset_box_layout.min_height,
            4 => &mut c.reset_box_layout.max_width,
            _ => &mut c.reset_box_layout.max_height,
        };
        *l = size_len(*l, vcx);
    }
}

fn size_len(l: Option<Len>, vcx: ValueCx<'_>) -> Option<Len> {
    let Some(Len::Anchor(id)) = l else { return l };
    let mut f = (*anchor_get(id, vcx)?).clone();
    fn change(f: &mut AnchorFn) {
        f.size = f.size.map(|s| match s {
            AnchorSize::Width => AnchorSize::Height,
            AnchorSize::Height => AnchorSize::Width,
            AnchorSize::Block => AnchorSize::Inline,
            AnchorSize::Inline => AnchorSize::Block,
            AnchorSize::SelfBlock => AnchorSize::SelfInline,
            AnchorSize::SelfInline => AnchorSize::SelfBlock,
            s => s,
        });
        for a in &mut f.alts {
            change(a);
        }
    }
    change(&mut f);
    f.fallback = size_len(f.fallback, vcx);
    Some(Len::Anchor(anchor_store(f, vcx)))
}
