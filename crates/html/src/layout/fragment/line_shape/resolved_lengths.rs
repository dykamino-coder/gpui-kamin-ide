//! Длины коробки в точках для клона поддерева вложенных рядов.

use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::style::computed::BoxStyle;
use crate::style::values::value::Len;

/// Клон поддерева с длинами коробки в ТОЧКАХ: `em`/`ch`/`ex`/`rem` в
/// размерах, полях, отбивках, рамках и вставках разрешены по кеглю своего
/// элемента (тем же `style::cascade::inherit` → `Computed::resolve_em`, что и при
/// отрисовке). `shape_full` читает голый стиль, и `margin-top: 2em` мерился
/// нулём (нестрогий `px_or`), а `height: 4em` — отказом всей стопки, хотя
/// рисунок кладёт их в точках. Наследуемые (`font-size`, `line-height`) не
/// трогаются: число в `line-height` наследуется множителем.
pub(crate) fn resolved_lengths(c: &Element, parent: &BoxStyle, vcx: ValueCx<'_>) -> Element {
    if vcx
        .lengths
        .is_some_and(|facts| facts.borrow_mut().unchanged(c))
    {
        return c.clone();
    }
    struct Frame<'a> {
        source: &'a Element,
        style: BoxStyle,
        output: Element,
        next: usize,
    }
    let enter = |source: &Element, parent: &BoxStyle| {
        let style = crate::style::cascade::materialize::box_style(parent, &source.style, vcx);
        let mut output = source.clone();
        if relative(&source.style.used()) {
            resolve(&mut output, &style);
        }
        (style, output)
    };
    let (style, output) = enter(c, parent);
    let mut pending = vec![Frame {
        source: c,
        style,
        output,
        next: 0,
    }];
    loop {
        let frame = pending.last_mut().unwrap();
        if frame.next == frame.source.children.len() {
            let done = pending.pop().unwrap().output;
            if let Some(parent) = pending.last_mut() {
                parent.output.children[parent.next - 1] = Node::Element(done);
            } else {
                return done;
            }
        } else {
            let node = &frame.source.children[frame.next];
            frame.next += 1;
            if let Node::Element(child) = node {
                if vcx
                    .lengths
                    .is_some_and(|facts| facts.borrow_mut().unchanged(child))
                {
                    continue;
                }
                let (style, output) = enter(child, &frame.style);
                pending.push(Frame {
                    source: child,
                    style,
                    output,
                    next: 0,
                });
            }
        }
    }
}

fn resolve(t: &mut Element, m: &BoxStyle) {
    fn fix(own: &mut Option<Len>, res: Option<Len>) {
        if own.is_some_and(|l| !matches!(l, Len::Px(_) | Len::Auto | Len::Pct(_)))
            && matches!(res, Some(Len::Px(_)))
        {
            *own = res;
        }
    }
    fn fix_sides(own: &mut crate::style::computed::Sides, res: &crate::style::computed::Sides) {
        fix(&mut own.top, res.top);
        fix(&mut own.right, res.right);
        fix(&mut own.bottom, res.bottom);
        fix(&mut own.left, res.left);
    }
    let style = t.style.used_mut();
    fix(&mut style.width, m.width);
    fix(&mut style.height, m.height);
    fix(&mut style.min_width, m.min_width);
    fix(&mut style.min_height, m.min_height);
    fix(&mut style.max_width, m.max_width);
    fix(&mut style.max_height, m.max_height);
    fix_sides(&mut style.margin, &m.margin);
    fix_sides(&mut style.padding, &m.padding);
    fix_sides(&mut style.border_width, &m.border_width);
    fix_sides(&mut style.inset, &m.inset);
}

/// Только эти длины меняет разрешение копии; проценты остаются отложенными.
fn relative(style: &crate::style::computed::Computed) -> bool {
    [
        style.width,
        style.height,
        style.min_width,
        style.min_height,
        style.max_width,
        style.max_height,
        style.margin.top,
        style.margin.right,
        style.margin.bottom,
        style.margin.left,
        style.padding.top,
        style.padding.right,
        style.padding.bottom,
        style.padding.left,
        style.border_width.top,
        style.border_width.right,
        style.border_width.bottom,
        style.border_width.left,
        style.inset.top,
        style.inset.right,
        style.inset.bottom,
        style.inset.left,
    ]
    .into_iter()
    .flatten()
    .any(|v| !matches!(v, Len::Px(_) | Len::Auto | Len::Pct(_)))
}

/// Доказательство отсутствия относительных длин удерживает проверенный COW-снимок.
#[derive(Default)]
pub(crate) struct LengthFacts {
    children: std::collections::HashMap<usize, (crate::dom::Children, bool)>,
}

impl LengthFacts {
    pub(crate) fn unchanged(&mut self, root: &Element) -> bool {
        let mut pending = vec![(&root.children, false)];
        while let Some((children, leave)) = pending.pop() {
            if self.children.contains_key(&children.identity()) {
                continue;
            }
            if leave {
                let unchanged = children.iter().all(|node| match node {
                    Node::Element(e) => {
                        !relative(&e.style.used()) && self.children[&e.children.identity()].1
                    }
                    _ => true,
                });
                self.children
                    .insert(children.identity(), (children.clone(), unchanged));
            } else {
                pending.push((children, true));
                for node in children.iter() {
                    if let Node::Element(e) = node {
                        pending.push((&e.children, false));
                    }
                }
            }
        }
        !relative(&root.style.used()) && self.children[&root.children.identity()].1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::table::anon::anon_element;

    #[test]
    fn resolved_copy_keeps_shared_children_and_invalidates_after_cow_edit() {
        let mut root = anon_element("div", vec![Node::Text("leaf".into())]);
        for _ in 0..2052 {
            root = anon_element("div", vec![Node::Element(root)]);
        }
        let mut facts = LengthFacts::default();
        assert!(facts.unchanged(&root));
        let entries = facts.children.len();
        let Node::Element(child) = &mut root.children[0] else {
            unreachable!()
        };
        child.style.used_mut().margin.top = Some(Len::Em(2.));
        assert!(!facts.unchanged(&root));
        assert_eq!(facts.children.len(), entries + 1);
        let doc = crate::document::state::DocState::new(Default::default());
        let cache = std::rc::Rc::new(std::cell::RefCell::new(LengthFacts::default()));
        let copy = resolved_lengths(
            &root,
            &BoxStyle::default(),
            doc.values().with_lengths(&cache),
        );
        let Node::Element(child) = &copy.children[0] else {
            unreachable!()
        };
        assert!(matches!(child.style.used().margin.top, Some(Len::Px(v)) if v > 0.));
        let Node::Element(original) = &root.children[0] else {
            unreachable!()
        };
        assert_eq!(original.style.used().margin.top, Some(Len::Em(2.)));
        assert_eq!(child.children.identity(), original.children.identity());
    }
}
