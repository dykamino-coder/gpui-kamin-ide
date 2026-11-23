//! Iterative enter/leave evaluation with whole-list reuse and staged output.
use super::{
    Transform,
    record::{Frame, Segment},
};
use crate::budget::admission::{Admission, Buffer, Error};
use crate::dom::{Children, Node, same_tree::same_box};
use std::rc::Rc;

pub(super) fn prepare<T: Transform>(
    transform: &T,
    input: &Children,
    context: T::Context,
    old: Option<Rc<Segment<T::Context>>>,
    admission: &Admission,
) -> Result<Rc<Segment<T::Context>>, Error> {
    admission.work(1)?;
    if let Some(old) = &old
        && old.matches(input, &context)
    {
        return Ok(old.clone());
    }
    let mut stack = Buffer::default();
    stack.push(
        Frame::new(input.clone(), context, old, admission)?,
        admission,
    )?;
    let mut completed = None;
    loop {
        if let Some(segment) = completed.take() {
            let segment: Rc<Segment<T::Context>> = segment;
            let Some(frame) = stack.last_mut() else {
                return Ok(segment);
            };
            let mut element = frame.pending.take().expect("entered element");
            element.children = segment.output.clone();
            transform.leave(&mut element, &frame.context, admission)?;
            let Node::Element(original) = &frame.input[frame.next - 1] else {
                unreachable!("entered element");
            };
            let changed = element.children.identity() != original.children.identity()
                || !same_box(&element, original);
            frame.append(changed.then_some(element), admission)?;
            frame.branches.push(segment, admission)?;
        }
        let frame = stack.last_mut().expect("active list");
        if frame.next == frame.input.len() {
            let mut frame = stack.pop().expect("completed list");
            let mut output = frame.output();
            transform.sequence(&mut output, &frame.context, admission)?;
            completed = Some(frame.finish(output, admission)?);
            continue;
        }
        admission.work(1)?;
        let index = frame.next;
        frame.next += 1;
        let Node::Element(original) = &frame.input[index] else {
            frame.append(None, admission)?;
            continue;
        };
        let mut element = original.clone();
        let child_context = transform.enter(&mut element, &frame.context, admission)?;
        let child_input = element.children.clone();
        let previous = frame.previous(&child_input);
        frame.pending = Some(element);
        if let Some(previous) = &previous
            && previous.matches(&child_input, &child_context)
        {
            completed = Some(previous.clone());
        } else {
            stack.push(
                Frame::new(child_input, child_context, previous, admission)?,
                admission,
            )?;
        }
    }
}
