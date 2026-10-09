//! Definite content widths passed through the HTML flow adapters.
//! CSS 2 sections 10.2-10.4 use the containing block to resolve and clamp percentages.

use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

pub(crate) fn inner(st: &Computed, outer: Option<f32>) -> Option<f32> {
    // Доли полей и отступов — от ширины содержащего блока (CSS 2.1 §8.3,
    // §8.4), то есть от `outer`. Прежде доля роняла всю цепочку в `None`, и
    // у детей процентные отступы считались от случайной базы
    // (`padding-percentage-inherit-001`: 6 точек вместо 30).
    let side = |l: Option<Len>| match l {
        None | Some(Len::Auto) => Some(0.0),
        Some(Len::Px(v)) => Some(v),
        Some(Len::Pct(k)) => outer.map(|o| k * o),
        _ => None,
    };
    match st.display {
        // Коробки нет — дети живут в потоке родителя.
        Some(Display::Contents) => return outer,
        None | Some(Display::Block) | Some(Display::ListItem) => {}
        _ => return None,
    }
    if st.vertical == Some(true) || st.column_count.is_some() || st.column_width.is_some() {
        return None;
    }
    let b = st.borders();
    let pb = side(st.padding.left)? + side(st.padding.right)? + side(b.left)? + side(b.right)?;
    let bb = st.border_box == Some(true);
    let w = match st.width {
        Some(Len::Px(w)) => {
            if bb {
                w - pb
            } else {
                w
            }
        }
        Some(Len::Pct(fraction)) => {
            // A percentage is definite when its containing block is definite.
            // Preserve deferred layout for out-of-flow and intrinsic limits.
            if matches!(
                st.position,
                Some(crate::style::computed::Position::Absolute | crate::style::computed::Position::Fixed)
            ) {
                return None;
            }
            let outer = outer?;
            let width = fraction * outer;
            if bb { width - pb } else { width }
        }
        None | Some(Len::Auto) => {
            if st.float.is_some_and(|f| f != 0)
                || matches!(
                    st.position,
                    Some(crate::style::computed::Position::Absolute)
                        | Some(crate::style::computed::Position::Fixed)
                )
            {
                return None;
            }
            
            outer? - side(st.margin.left)? - side(st.margin.right)? - pb
        }
        _ => return None,
    };
    // CSS 2 section 10.4 applies these bounds to every tentative width,
    // including a pixel width or the fill width of an auto block.
    let bound = |length| match length {
        None | Some(Len::Auto) => Some(None),
        Some(Len::Px(value)) => Some(Some(value)),
        Some(Len::Pct(value)) => outer.map(|outer| Some(value * outer)),
        _ => None,
    };
    let content_bound = |value: f32| (value - if bb { pb } else { 0.0 }).max(0.0);
    let minimum = bound(st.min_width)?.map(content_bound).unwrap_or(0.0);
    let maximum = bound(st.max_width)?
        .map(content_bound)
        .unwrap_or(f32::INFINITY);
    let w = w.min(maximum).max(minimum);
    (w > 0.0).then_some(w)
}
