//! Общие ограничения и пиксельные промежутки при раскрытии flex в потоки.

use crate::dom::Element;
use crate::layout::page::paged::visible_overflow;
use crate::render::out_of_flow;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

pub(super) fn zero(l: &Option<Len>) -> bool {
    match l {
        None => true,
        Some(Len::Px(v)) => v.abs() < 0.01,
        _ => false,
    }
}

pub(super) fn plain_container(c: &Element) -> bool {
    let s = &c.style;
    let b = s.borders();
    if c.inline
        || s.display != Some(Display::Flex)
        || s.webkit_box == Some(true)
        || s.flex_wrap != Some(true)
        || s.flex_wrap_reverse == Some(true)
        || s.flex_balance == Some(true)
        || s.wm().vertical()
        || s.justify_content.is_some()
        || s.align_content.is_some()
        || s.align_items.is_some()
        // `position: relative` без сдвигов ничего не двигает, а содержащим
        // блоком ему служить некому (внепоточных детей гейт не пускает).
        || !(s.position.is_none()
            || (s.position == Some(crate::style::computed::Position::Relative)
                && [&s.inset.top, &s.inset.right, &s.inset.bottom, &s.inset.left]
                    .into_iter()
                    .all(|l| matches!(l, None | Some(Len::Auto)))))
        || s.transform.is_some()
        || s.min_height.is_some()
        || s.max_height.is_some()
        || !visible_overflow(s)
        || ![
            &s.margin.top,
            &s.margin.bottom,
            &s.margin.left,
            &s.margin.right,
            &s.padding.top,
            &s.padding.bottom,
            &s.padding.left,
            &s.padding.right,
            &b.top,
            &b.bottom,
            &b.left,
            &b.right,
        ]
        .into_iter()
        .all(zero)
    {
        return false;
    }
    true
}

pub(super) fn plain_item(k: &Element) -> bool {
    let s = &k.style;
    !k.inline
        && !out_of_flow(s)
        && matches!(
            s.position,
            None | Some(crate::style::computed::Position::Relative)
        )
        && s.float.unwrap_or(0) == 0
        && !s.flex_grow.is_some_and(|g| g > 0.0)
        && s.align_self.is_none()
        && !s.align_self_normal
        && zero(&s.margin.left)
        && zero(&s.margin.right)
}

pub(super) fn gaps(s: &Computed) -> Option<(f32, f32)> {
    let px = |l: Option<Len>| match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(v.max(0.0)),
        _ => None,
    };
    match s.gap {
        None => Some((0.0, 0.0)),
        Some((r, g)) => Some((px(r)?, px(g)?)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gaps_default_clamp_negatives_and_refuse_unresolved_units() {
        let mut s = Computed::default();
        assert_eq!(gaps(&s), Some((0.0, 0.0)));
        s.gap = Some((Some(Len::Px(-2.0)), Some(Len::Px(3.0))));
        assert_eq!(gaps(&s), Some((0.0, 3.0)));
        s.gap = Some((None, Some(Len::Auto)));
        assert_eq!(gaps(&s), None);
    }
}
