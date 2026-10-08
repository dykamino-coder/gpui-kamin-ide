//! Determine whether height constraints separate a block from its end margins.

use super::*;

pub(super) fn separate(e: &mut Element) -> bool {
    if !lowers(e) {
        return false;
    }
    // The child's end margin cannot change this constrained parent's used
    // height. Leaving a negative margin in the flex adapter would shrink the
    // auto height again, undoing the constraint that separated the margins.
    for node in &mut e.children {
        if let Node::Element(child) = node
            && in_flow(&child.style)
        {
            pin_inherited_margins(child, false, true);
            child.style.margin.bottom = Some(Len::Px(0.0));
        }
    }
    true
}

/// CSS 2.2 §10.7 reruns height and margin calculation with the limiting
/// height. Blink block_layout_algorithm.cc:1459-1466 likewise separates
/// the end margin when the used block size differs from the intrinsic size.
/// Unknown content remains on the existing path; this proof uses only
/// one definite block child whose border edge can be calculated here.
pub(super) fn lowers(e: &Element) -> bool {
    let Some(mut cap) = margin_px(e.style.max_height, &e.style) else {
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
    if e.style.border_box == Some(true) {
        let Some(inset) = edges(&e.style) else {
            return false;
        };
        cap = (cap - inset).max(0.0);
    }
    if let Some(min) = margin_px(e.style.min_height, &e.style) {
        cap = cap.max(min);
    } else if !zero_len(e.style.min_height) {
        return false;
    }
    let mut bottom = None;
    for node in &e.children {
        let child = match node {
            Node::Text(text) if blank_text(text) => continue,
            Node::Text(_) => return false,
            Node::Element(child) if !in_flow(&child.style) => continue,
            Node::Element(child) => child,
        };
        if bottom.is_some()
            || inline_level_box(child)
            || child.style.clear.is_some()
            || child.style.min_height.is_some()
            || child.style.max_height.is_some()
        {
            return false;
        }
        let Some(height) = margin_px(child.style.height, &child.style) else {
            return false;
        };
        let Some(inset) = edges(&child.style) else {
            return false;
        };
        let Some(top) = margin_or_bail(child.style.margin.top, &child.style) else {
            return false;
        };
        let height = if child.style.border_box == Some(true) {
            height.max(inset)
        } else {
            height + inset
        };
        bottom = Some(top + height);
    }
    bottom.is_some_and(|bottom| bottom > cap)
}

pub(super) fn raises(e: &Element) -> bool {
    let zero = |l: Option<Len>| matches!(l, None | Some(Len::Px(0.0)) | Some(Len::Pct(0.0)));
    match margin_px(e.style.min_height, &e.style) {
        None => !zero(e.style.min_height),
        Some(mh) if mh <= 0.0 => false,
        Some(mh) => {
            let mut sum = 0.0f32;
            let mut known = true;
            for c in &e.children {
                match c {
                    Node::Text(t) if blank_text(t) => {}
                    Node::Text(_) => known = false,
                    Node::Element(ch) if !in_flow(&ch.style) => {}
                    Node::Element(ch) if ch.inline && inline_level_box(ch) => known = false,
                    Node::Element(ch) => {
                        let b = ch.style.borders();
                        let side = |l: Option<Len>| match l {
                            None => Some(0.0),
                            Some(Len::Px(v)) => Some(v),
                            _ => None,
                        };
                        let own = match (
                            margin_px(ch.style.height, &ch.style),
                            side(ch.style.padding.top),
                            side(ch.style.padding.bottom),
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
            !known || mh > sum + 0.01
        }
    }
}
