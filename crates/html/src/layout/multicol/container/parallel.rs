//! Independent break growth for positioned flows in continuous block coordinates.
//!
//! Resolve the original containing-block position before inserting fragmentation
//! space. Growing an auto-sized abspos must not resolve its bottom inset again.

use crate::dom::{Element, Node};
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::fragment::push::grow_pushed;
use crate::layout::fragment::{Shape, ShapeCx};
use crate::layout::multicol::spanner::multicol_container;
use crate::layout::positioned::predicates::OOF_OWN;
use crate::render::out_of_flow;
use crate::style::computed::{Display, Position};
use crate::style::values::value::Len;
use crate::text::inline::establishes_cb;

fn measure(c: &Element) -> Option<(Shape, f32)> {
    let s = shape_full(c, 4, ShapeCx::COLUMNS)?;
    let own = OOF_OWN.with(|m| {
        m.borrow()
            .get(&c.node_id)
            .filter(|&&(_, full)| (full - s.0).abs() < 0.01)
            .map_or(s.0, |&(own, _)| own)
    });
    Some((s, own))
}

fn zero(v: Option<Len>) -> bool {
    matches!(v, None | Some(Len::Px(0.0)))
}

// Other formatting contexts own their block offsets. They need a positioning
// adapter before their descendants can participate in this block-flow pass.
fn block(c: &Element) -> bool {
    let border = c.style.borders();
    !c.inline
        && matches!(c.style.display, None | Some(Display::Block))
        && !multicol_container(&c.style)
        && !c.style.bdb_clone
        && c.style.float.unwrap_or(0) == 0
        && zero(c.style.padding.top)
        && zero(c.style.padding.bottom)
        && zero(border.top)
        && zero(border.bottom)
}

fn positioned(
    c: &mut Element,
    cb_height: f32,
    static_y: f32,
    origin: f32,
    h: f32,
    count: usize,
    copies: usize,
) -> bool {
    if c.style.position != Some(Position::Absolute)
        || !block(c)
        || !matches!(c.style.height, None | Some(Len::Auto))
        || !zero(c.style.margin.top)
        || !zero(c.style.margin.bottom)
    {
        return false;
    }
    let Some((s, _)) = measure(c) else {
        return false;
    };
    if s.3.is_empty() || (s.4.is_empty() && s.5.is_empty()) {
        return false;
    }
    let used = match (c.style.inset.top, c.style.inset.bottom) {
        (Some(Len::Px(top)), None | Some(Len::Auto)) => top,
        (None | Some(Len::Auto), Some(Len::Px(bottom))) => cb_height - bottom - s.0,
        (None | Some(Len::Auto), None | Some(Len::Auto)) => static_y,
        _ => return false,
    };
    let phase = (origin + used).rem_euclid(h);
    let original = c.style.clone();
    let mut probe = c.clone();
    probe.style.position = Some(Position::Relative);
    probe.style.inset.top = None;
    probe.style.inset.bottom = None;
    probe.style.margin.top = Some(Len::Px(phase));
    let mut shape = s.clone();
    shape.1 = phase;
    let planned = copies.max(((phase + s.0) / h).ceil() as usize + count);
    let mut grown = grow_pushed(vec![(probe, shape)], count, Some(h), None, planned, &[]);
    let (mut probe, after) = grown.remove(0);
    if after.0 <= s.0 + 0.01 {
        return false;
    }
    probe.style = original;
    probe.style.inset.top = Some(Len::Px(used));
    probe.style.inset.bottom = Some(Len::Auto);
    *c = probe;
    true
}

fn walk(c: &mut Element, origin: f32, h: f32, count: usize, copies: usize) -> bool {
    if !block(c) {
        return false;
    }
    let Some((_, own)) = measure(c) else {
        return false;
    };
    let cb = establishes_cb(&c.style);
    let (mut cursor, mut previous) = (0.0f32, 0.0f32);
    let mut changed = false;
    for n in &mut c.children {
        let Node::Element(k) = n else { continue };
        if out_of_flow(&k.style) {
            if cb {
                changed |= positioned(k, own, cursor, origin, h, count, copies);
            }
            continue;
        }
        let Some((s, height)) = measure(k) else {
            continue;
        };
        let start = cursor + previous.max(s.1);
        changed |= walk(k, origin + start, h, count, copies);
        cursor = start + height;
        previous = s.2;
    }
    changed
}

pub(super) fn grow(kids: &mut [(Element, Shape)], h: f32, count: usize, copies: usize) {
    let (mut cursor, mut previous) = (0.0f32, 0.0f32);
    for (c, s) in kids {
        let own = measure(c).map_or(s.0, |(_, own)| own);
        let start = cursor + previous.max(s.1);
        if c.attr(crate::layout::fragment::paginate::MARK).is_none()
            && walk(c, start, h, count, copies)
            && let Some(next) = shape_full(c, 4, ShapeCx::COLUMNS)
        {
            *s = (next.0, s.1, s.2, next.3, next.4, next.5);
        }
        if !out_of_flow(&c.style) {
            cursor = start + own;
            previous = s.2;
        }
    }
}

#[cfg(test)]
mod tests {
    //! Fragment growth preserves the original CB position and its normal-flow size.

    use super::*;
    use crate::style::computed::Computed;

    fn element(style: Computed, children: Vec<Node>) -> Element {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(10_000);
        Element {
            node_id: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            tag: "div".into(),
            style,
            children,
            attrs: Vec::new(),
            inline: false,
            list_item: None,
            anim: None,
            hover: None,
            first_letter: None,
            first_line: None,
        }
    }

    fn containing_block() -> Element {
        let children = [(90.0, false), (50.0, true), (50.0, false)]
            .into_iter()
            .map(|(h, avoid)| {
                Node::Element(element(
                    Computed {
                        height: Some(Len::Px(h)),
                        break_inside_avoid: avoid,
                        ..Default::default()
                    },
                    Vec::new(),
                ))
            })
            .collect();
        let mut abs = element(
            Computed {
                position: Some(Position::Absolute),
                ..Default::default()
            },
            children,
        );
        abs.style.inset.bottom = Some(Len::Px(0.0));
        element(
            Computed {
                height: Some(Len::Px(190.0)),
                position: Some(Position::Relative),
                ..Default::default()
            },
            vec![Node::Element(abs)],
        )
    }

    #[test]
    fn bottom_position_is_resolved_before_auto_height_grows() {
        let mut cb = containing_block();
        assert!(walk(&mut cb, 0.0, 100.0, 2, 4));
        let Node::Element(abs) = &cb.children[0] else {
            panic!()
        };
        assert_eq!(abs.style.inset.top, Some(Len::Px(0.0)));
        assert_eq!(measure(abs).unwrap().0.0, 200.0);
        let (shape, own) = measure(&cb).unwrap();
        assert_eq!(shape.0, 200.0);
        assert_eq!(own, 190.0);
    }

    #[test]
    fn independent_breaks_use_the_containing_blocks_fragmentainer_phase() {
        let mut cb = containing_block();
        assert!(walk(&mut cb, 80.0, 100.0, 2, 4));
        let Node::Element(abs) = &cb.children[0] else {
            panic!()
        };
        assert_eq!(abs.style.inset.top, Some(Len::Px(0.0)));
        assert_eq!(measure(abs).unwrap().0.0, 220.0);
        assert_eq!(measure(&cb).unwrap().1, 190.0);
    }
}
