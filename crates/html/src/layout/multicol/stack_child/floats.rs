//! Нулевая normal-flow коробка float-хоста по CSS 2.1 §10.6.3 в блочной оси.

use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::layout::block::struts::through_strut;
use crate::layout::fragment::flow::{empty_floats, logical_copy};
use crate::style::values::value::Len;

pub(super) fn normalize(
    copy: &mut Element,
    vertical: bool,
    reverse: bool,
    vcx: ValueCx<'_>,
) -> (bool, bool) {
    let logical = vertical.then(|| logical_copy(copy, reverse)).flatten();
    let flow = logical.as_ref().unwrap_or(copy);
    let empty = empty_floats(flow);
    // Поля уже вынесены в стопку. Флоаты не дают хосту normal-flow размера,
    // а их margin box остаётся переполнением BandFlow. Нельзя растянуть фон
    // пустого хоста до конца float; clear или рамка препятствуют сквозной струне.
    let zero = (!vertical || logical.is_some())
        && (empty || (!vertical && crate::layout::float::float_only_box(flow).is_some()))
        && through_strut(flow, vcx).is_some();
    if zero {
        if vertical {
            copy.style.used_mut().width = Some(Len::Px(0.0));
        } else {
            copy.style.used_mut().height = Some(Len::Px(0.0));
        }
    }
    (empty, zero)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::Node;

    fn host(nodes: &[Node]) -> &Element {
        nodes
            .iter()
            .find_map(|n| match n {
                Node::Element(e) if e.tag == "div" => Some(e),
                Node::Element(e) => Some(host(&e.children)),
                _ => None,
            })
            .expect("authored host")
    }

    #[test]
    fn float_only_background_stays_zero_on_the_physical_block_axis() {
        for (mode, vertical, reverse) in [
            ("horizontal-tb", false, false),
            ("vertical-lr", true, false),
            ("vertical-rl", true, true),
        ] {
            for tail in ["", "<div style='clear:both;width:20px;height:20px'></div>"] {
                let tree = crate::dom::parse(
                    &format!(
                        "<div style='writing-mode:{mode}'><div style='float:left;width:70px;height:40px'></div>{tail}</div>"
                    ),
                    "",
                );
                let mut copy = host(&tree).clone();
                let (empty, zero) = normalize(&mut copy, vertical, reverse, tree.values());
                assert!(empty, "{mode}: {tail}");
                assert_eq!(zero, tail.is_empty(), "{mode}: {tail}");
                if zero {
                    assert_eq!(
                        if vertical {
                            copy.style.computed().width
                        } else {
                            copy.style.computed().height
                        },
                        Some(Len::Px(0.0))
                    );
                    assert_eq!(
                        if vertical {
                            copy.style.computed().height
                        } else {
                            copy.style.computed().width
                        },
                        None
                    );
                }
            }
        }
    }
}
