//! Перестановка входов расчёта размеров при перпендикулярной try-тактике.

use crate::document::state::ValueCx;
use crate::style::computed::Computed;
use crate::style::css::LonghandId as L;
use crate::style::values::value::{AnchorFn, AnchorSize, Len, anchor_get, anchor_store};

pub(super) fn transpose(c: &mut Computed, vcx: ValueCx<'_>) {
    std::mem::swap(&mut c.width, &mut c.height);
    std::mem::swap(&mut c.min_width, &mut c.min_height);
    std::mem::swap(&mut c.max_width, &mut c.max_height);
    for (a, b) in [
        (L::Width, L::Height),
        (L::MinWidth, L::MinHeight),
        (L::MaxWidth, L::MaxHeight),
    ] {
        let av = c.inherit_longhands.contains(a);
        let bv = c.inherit_longhands.contains(b);
        c.inherit_longhands.set(a, bv);
        c.inherit_longhands.set(b, av);
    }
    c.calc_size.swap(0, 1);
    c.calc_size.swap(2, 3);
    c.calc_size_max.swap(0, 1);
    c.calc_size_basis.swap(0, 1);
    c.calc_size_basis.swap(2, 3);
    c.calc_size_basis.swap(4, 5);
    c.stretch_size.swap(0, 1);
    c.stretch_size.swap(2, 4);
    c.stretch_size.swap(3, 5);
    for l in [
        &mut c.width,
        &mut c.height,
        &mut c.min_width,
        &mut c.min_height,
        &mut c.max_width,
        &mut c.max_height,
    ] {
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
