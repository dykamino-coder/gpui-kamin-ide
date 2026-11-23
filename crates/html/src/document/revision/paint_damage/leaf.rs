//! A text leaf has no descendants to recascade when only its ink changes.
//! Generated children and first-line/letter effects keep the general fallback.

use super::{PaintValue, own_paint_only};
use crate::dom::{Element, Node};

pub(in crate::document::revision) fn color(a: &Element, b: &Element) -> Option<PaintValue> {
    if a.node_id != b.node_id || a.tag != b.tag || super::CONTROLS.contains(&a.tag.as_str()) {
        return None;
    }
    for e in [a, b] {
        if e.children.is_empty()
            || !e.children.iter().all(|n| matches!(n, Node::Text(_)))
            || e.first_letter.is_some()
            || e.first_line.is_some()
        {
            return None;
        }
    }
    let (old, new) = (a.style.cascaded(), b.style.cascaded());
    if old.inh.ui.color == new.inh.ui.color || old.reset_svg_math != new.reset_svg_math {
        return None;
    }
    let mut normalized = old.clone();
    normalized.inh.ui.color = new.inh.ui.color;
    own_paint_only(&normalized, new).map(|bits| bits.union(PaintValue::COLOR))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::Computed;
    use crate::style::values::value::{Color, Len};

    fn leaf(style: Computed) -> Element {
        Element::synthetic(
            "span",
            style,
            Default::default(),
            vec![Node::Text("ink".into())],
        )
    }

    #[test]
    fn leaf_ink_is_paint_only_but_other_ui_layout_and_children_are_not() {
        let old = Computed::default();
        let mut new = old.clone();
        new.inh.ui.color = Some(Color {
            r: 1.,
            g: 0.,
            b: 0.,
            a: 1.,
        });
        assert_eq!(
            color(&leaf(old.clone()), &leaf(new.clone())),
            Some(PaintValue::COLOR)
        );
        let mut cursor = new.clone();
        cursor.inh.ui.cursor = Some("pointer".into());
        assert_eq!(color(&leaf(old.clone()), &leaf(cursor)), None);
        let mut layout = new.clone();
        layout.reset_box_layout.margin.top = Some(Len::Px(1.));
        assert_eq!(color(&leaf(old.clone()), &leaf(layout)), None);
        let mut parent = leaf(new);
        parent.children = vec![Node::Element(leaf(Computed::default()))].into();
        assert_eq!(color(&leaf(old), &parent), None);
    }
}
