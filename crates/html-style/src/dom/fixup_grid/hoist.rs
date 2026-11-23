//! Подъём размещённых absolute-коробок к сетке с сохранением DOM-родителя стиля.
use crate::budget::admission::{Admission, Buffer, Error};
use crate::dom::normalization::{Transform, apply};
use crate::dom::{Children, Element, Node, own_containing_block};
use crate::style::computed::{Computed, Display, Position};

pub(crate) fn hoist_grid_abspos(
    admission: &Admission,
    nodes: Vec<Node>,
) -> Result<Vec<Node>, Error> {
    apply(&HoistGridAbspos, nodes, (), admission)
}

pub(crate) struct HoistGridAbspos;
impl Transform for HoistGridAbspos {
    type Context = ();
    fn enter(&self, _: &mut Element, _: &(), _: &Admission) -> Result<(), Error> {
        Ok(())
    }
    fn leave(&self, el: &mut Element, _: &(), admission: &Admission) -> Result<(), Error> {
        if !is_grid(el.style.cascaded()) || !own_containing_block(el.style.cascaded()) {
            return Ok(());
        }
        let mut taken = Children::default();
        for i in 0..el.children.len() {
            let Node::Element(child) = &el.children[i] else {
                continue;
            };
            if own_containing_block(child.style.cascaded()) || is_grid(child.style.cascaded()) {
                continue;
            }
            let Node::Element(child) = &mut el.children.edit(admission, 0)?[i] else {
                unreachable!()
            };
            steal_placed(admission, child, &mut taken)?;
        }
        if !taken.is_empty() {
            el.children
                .edit(admission, taken.len())?
                .extend(taken.into_vec());
        }
        Ok(())
    }
}

/// Собственный CB и подсетка замыкают область переноса (css-grid-2 §9).
/// Меняется родитель коробки; наследование остаётся у непосредственного owner.
fn steal_placed(
    admission: &Admission,
    owner: &mut Element,
    out: &mut Children,
) -> Result<(), Error> {
    struct Frame {
        remaining: std::vec::IntoIter<Node>,
        kept: Children,
        owner: Option<Element>,
        id: u64,
        lent: bool,
    }
    fn take_children(
        el: &mut Element,
        admission: &Admission,
    ) -> Result<std::vec::IntoIter<Node>, Error> {
        el.children.edit(admission, 0)?;
        Ok(std::mem::take(&mut el.children).into_iter())
    }
    fn new_frame(
        remaining: std::vec::IntoIter<Node>,
        owner: Option<Element>,
        id: u64,
        admission: &Admission,
    ) -> Result<Frame, Error> {
        let mut kept = Children::default();
        kept.edit(admission, remaining.len())?;
        Ok(Frame {
            remaining,
            kept,
            owner,
            id,
            lent: false,
        })
    }
    let mut stack = Buffer::default();
    let remaining = take_children(owner, admission)?;
    stack.push(
        new_frame(remaining, None, owner.node_id, admission)?,
        admission,
    )?;
    while let Some(frame) = stack.last_mut() {
        if let Some(mut node) = frame.remaining.next() {
            admission.work(1)?;
            if let Node::Element(el) = &mut node {
                let placed = el.style.cascaded().reset_grid.grid_col.is_some()
                    || el.style.cascaded().reset_grid.grid_row.is_some();
                if el.style.cascaded().reset_positioned.position == Some(Position::Absolute)
                    && placed
                {
                    if frame.id != 0 {
                        el.marks.style_parent = Some(frame.id);
                        frame.lent = true;
                    }
                    if out.len() == out.capacity() {
                        out.edit(admission, out.len().max(4))?;
                    }
                    out.edit(admission, 1)?.push(node);
                    continue;
                }
                if !own_containing_block(el.style.cascaded())
                    && !el.style.cascaded().reset_grid.subgrid
                {
                    let Node::Element(mut el) = node else {
                        unreachable!()
                    };
                    let remaining = take_children(&mut el, admission)?;
                    let id = el.node_id;
                    stack.push(new_frame(remaining, Some(el), id, admission)?, admission)?;
                    continue;
                }
            }
            frame.kept.edit(admission, 1)?.push(node);
        } else {
            let frame = stack.pop().expect("active hoist frame");
            if let Some(mut el) = frame.owner {
                el.children = frame.kept;
                el.marks.lends_style |= frame.lent;
                stack
                    .last_mut()
                    .expect("hoist parent")
                    .kept
                    .edit(admission, 1)?
                    .push(Node::Element(el));
            } else {
                owner.children = frame.kept;
                owner.marks.lends_style |= frame.lent;
            }
        }
    }
    Ok(())
}

fn is_grid(c: &Computed) -> bool {
    matches!(
        c.reset_box_layout.display,
        Some(Display::Grid | Display::InlineGrid)
    )
}
