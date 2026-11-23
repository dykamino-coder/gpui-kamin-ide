//! Full and retained evaluation share one interleaved sibling cursor.
use super::{
    Transform,
    frame::{Frame, Owner},
    record::Segment,
};
use crate::budget::admission::{Admission, Buffer, Error};
use crate::dom::{Children, Element, Node};
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
    let result = evaluate(transform, input.clone(), context, true, old, admission)?;
    Ok(result.segment.expect("recorded root"))
}

pub(super) struct Evaluation<C> {
    pub output: Children,
    pub segment: Option<Rc<Segment<C>>>,
}

pub(super) fn evaluate<T: Transform>(
    transform: &T,
    input: Children,
    context: T::Context,
    recorded: bool,
    old: Option<Rc<Segment<T::Context>>>,
    admission: &Admission,
) -> Result<Evaluation<T::Context>, Error> {
    let mut stack = Buffer::default();
    stack.push(
        start(
            transform,
            Owner::root(input),
            context,
            recorded,
            old,
            admission,
        )?,
        admission,
    )?;
    while let Some(frame) = stack.last_mut() {
        if frame.next == frame.owner.nodes().len() {
            let mut frame = stack.pop().expect("completed list");
            transform.sequence(frame.owner.nodes_mut(), &frame.context, admission)?;
            let segment = frame
                .recording
                .take()
                .map(|recording| {
                    recording.finish(frame.context, frame.owner.nodes().clone(), admission)
                })
                .transpose()?;
            if let Some(element) = frame.owner.element.take() {
                complete(
                    transform,
                    element,
                    segment,
                    stack.last_mut().expect("parent"),
                    admission,
                )?;
            } else {
                return Ok(Evaluation {
                    output: frame.owner.into_root(),
                    segment,
                });
            }
            continue;
        }
        admission.work(1)?;
        if !matches!(frame.owner.nodes()[frame.next], Node::Element(_))
            || !transform.visit_child(frame.owner.nodes(), frame.next, &frame.context)
        {
            advance(transform, frame, admission)?;
            continue;
        }
        let mut element = frame.take_element();
        let context = transform.enter(&mut element, &frame.context, admission)?;
        let previous = frame.previous(&element.children);
        if let Some(previous) = &previous
            && previous.matches(&element.children, &context)
        {
            element.children = previous.output.clone();
            complete(transform, element, Some(previous.clone()), frame, admission)?;
        } else {
            stack.push(
                start(
                    transform,
                    Owner::element(element),
                    context,
                    recorded,
                    previous,
                    admission,
                )?,
                admission,
            )?;
        }
    }
    unreachable!("root always completes")
}

fn start<T: Transform>(
    transform: &T,
    owner: Owner,
    context: T::Context,
    recorded: bool,
    old: Option<Rc<Segment<T::Context>>>,
    admission: &Admission,
) -> Result<Frame<T::Context>, Error> {
    let mut frame = Frame::new(owner, context, recorded, old, admission)?;
    transform.before_list(frame.owner.nodes_mut(), &frame.context, admission)?;
    Ok(frame)
}

fn complete<T: Transform>(
    transform: &T,
    mut element: Element,
    segment: Option<Rc<Segment<T::Context>>>,
    parent: &mut Frame<T::Context>,
    admission: &Admission,
) -> Result<(), Error> {
    transform.leave(&mut element, &parent.context, admission)?;
    parent.restore(element, segment, admission)?;
    advance(transform, parent, admission)
}
fn advance<T: Transform>(
    transform: &T,
    frame: &mut Frame<T::Context>,
    admission: &Admission,
) -> Result<(), Error> {
    frame.next = transform.after_child(
        frame.owner.nodes_mut(),
        frame.next,
        &frame.context,
        admission,
    )?;
    assert!(
        frame.next <= frame.owner.nodes().len(),
        "transform cursor outside edited list"
    );
    Ok(())
}
