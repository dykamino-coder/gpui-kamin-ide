//! Доказуемые высоты блоков при схлопывании полей: когда высота не равна высоте содержимого,
//! поля ребёнка и родителя не сливаются (CSS 2.2 §10.7, Blink `block_layout_algorithm.cc`).
//! `lowers` — `max-height` режет единственного определённого ребёнка; `raises` — `min-height`
//! тянет коробку выше содержимого; `separate` обнуляет нижние поля детей. Нулевая высота блока
//! из одних флоатов — `zero_floats`.

use crate::dom::whitespace::blank_text;
use crate::dom::{Element, Node};
use crate::layout::block::struts::{margin_or_bail, margin_px, pin_inherited_margins, zero_len};
use crate::layout::box_kind::{in_flow, inline_level_box};
use crate::style::computed::Computed;
use crate::style::values::value::Len;

mod zero_floats;
pub(crate) use zero_floats::{used_style, zero_float_blocks};

pub(super) fn separate(e: &mut Element, vcx: crate::document::state::ValueCx<'_>) -> bool {
    if !lowers(e, vcx) {
        return false;
    }
    // The child's end margin cannot change this constrained parent's used
    // height. Leaving a negative margin in the flex adapter would shrink the
    // auto height again, undoing the constraint that separated the margins.
    for node in &mut e.children {
        if let Node::Element(child) = node
            && in_flow(&child.style.used())
        {
            pin_inherited_margins(child, false, true, vcx);
            child.style.used_mut().margin.bottom = Some(Len::Px(0.0));
        }
    }
    true
}

/// CSS 2.2 §10.7 reruns height and margin calculation with the limiting
/// height. Blink block_layout_algorithm.cc:1459-1466 likewise separates
/// the end margin when the used block size differs from the intrinsic size.
/// Unknown content remains on the existing path; this proof uses only
/// one definite block child whose border edge can be calculated here.
pub(super) fn lowers(e: &Element, vcx: crate::document::state::ValueCx<'_>) -> bool {
    let st = e.style.used();
    let Some(mut cap) = margin_px(st.max_height, &st, vcx) else {
        return false;
    };
    let edges = |style: &Computed| -> Option<f32> {
        let b = style.borders();
        [style.padding.top, style.padding.bottom, b.top, b.bottom]
            .into_iter()
            .try_fold(0.0, |sum, value| match value {
                None => Some(sum),
                Some(Len::Px(v)) => Some(sum + v),
                _ => None,
            })
    };
    if st.border_box == Some(true) {
        let Some(inset) = edges(&st) else {
            return false;
        };
        cap = (cap - inset).max(0.0);
    }
    if let Some(min) = margin_px(st.min_height, &st, vcx) {
        cap = cap.max(min);
    } else if !zero_len(st.min_height) {
        return false;
    }
    let mut bottom = None;
    for node in &e.children {
        let child = match node {
            Node::Text(text) if blank_text(text) => continue,
            Node::Text(_) => return false,
            Node::Element(child) if !in_flow(&child.style.used()) => continue,
            Node::Element(child) => child,
        };
        let cs = child.style.used();
        if bottom.is_some()
            || inline_level_box(child)
            || cs.clear.is_some()
            || cs.min_height.is_some()
            || cs.max_height.is_some()
        {
            return false;
        }
        let Some(height) = margin_px(cs.height, &cs, vcx) else {
            return false;
        };
        let Some(inset) = edges(&cs) else {
            return false;
        };
        let Some(top) = margin_or_bail(cs.margin.top, &cs, vcx) else {
            return false;
        };
        let height = if cs.border_box == Some(true) {
            height.max(inset)
        } else {
            height + inset
        };
        bottom = Some(top + height);
    }
    bottom.is_some_and(|bottom| bottom > cap)
}

pub(super) fn raises(e: &Element, vcx: crate::document::state::ValueCx<'_>) -> bool {
    let st = e.style.used();
    let zero = |l: Option<Len>| matches!(l, None | Some(Len::Px(0.0)) | Some(Len::Pct(0.0)));
    match margin_px(st.min_height, &st, vcx) {
        None => !zero(st.min_height),
        Some(mh) if mh <= 0.0 => false,
        Some(mh) => {
            let mut sum = 0.0f32;
            let mut known = true;
            for c in &e.children {
                match c {
                    Node::Text(t) if blank_text(t) => {}
                    Node::Text(_) => known = false,
                    Node::Element(ch) if !in_flow(&ch.style.used()) => {}
                    Node::Element(ch) if ch.inline && inline_level_box(ch) => known = false,
                    Node::Element(ch) => {
                        let cs = ch.style.used();
                        let b = cs.borders();
                        let side = |l: Option<Len>| match l {
                            None => Some(0.0),
                            Some(Len::Px(v)) => Some(v),
                            _ => None,
                        };
                        let own = match (
                            margin_px(cs.height, &cs, vcx),
                            side(cs.padding.top),
                            side(cs.padding.bottom),
                            side(b.top),
                            side(b.bottom),
                        ) {
                            (Some(h), Some(pt), Some(pb), Some(bt), Some(bb)) => {
                                Some(h + pt + pb + bt + bb)
                            }
                            _ => None,
                        };
                        match own {
                            Some(v) => sum += v,
                            None => known = false,
                        }
                    }
                }
                if !known {
                    break;
                }
            }
            !known || mh > sum + crate::tolerance::COORD_EPS
        }
    }
}
