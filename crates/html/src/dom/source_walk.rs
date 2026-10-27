//! Итеративное чтение исходного DOM; память зависит от глубины, а не ширины.

use crate::budget::admission::{Admission, Buffer, Error};
use markup5ever_rcdom::{Handle, NodeData};

#[derive(Clone, Copy)]
pub(crate) enum Templates {
    None,
    All,
    Active,
}

struct Frame {
    node: Handle,
    next: usize,
    template_seen: bool,
    allow_template: bool,
    shadow_seen: bool,
}

impl Frame {
    fn new(node: Handle, allow_template: bool) -> Self {
        Self {
            node,
            next: 0,
            template_seen: false,
            allow_template,
            shadow_seen: false,
        }
    }
}

/// Прежний API обходов представления; bounded ранние проходы вызывают `try_next`.
pub(crate) fn descendants(root: &Handle, templates: bool) -> SourceWalk {
    SourceWalk::new(
        root,
        if templates {
            Templates::All
        } else {
            Templates::None
        },
        Admission::default(),
    )
    .expect("standalone DOM walk allocation")
}

pub(crate) struct SourceWalk {
    frames: Buffer<Frame>,
    first: bool,
    templates: Templates,
    admission: Admission,
}

impl SourceWalk {
    pub(crate) fn new(
        root: &Handle,
        templates: Templates,
        admission: Admission,
    ) -> Result<Self, Error> {
        let mut frames = Buffer::default();
        frames.push(
            Frame::new(root.clone(), matches!(templates, Templates::All)),
            &admission,
        )?;
        Ok(Self {
            frames,
            first: true,
            templates,
            admission,
        })
    }

    /// Отказ терминален: кадры освобождены, следующие вызовы не расходуют work.
    pub(crate) fn try_next(&mut self) -> Result<Option<Handle>, Error> {
        let result = self.step();
        if result.is_err() {
            self.frames = Buffer::default();
            self.first = false;
        }
        result
    }

    fn step(&mut self) -> Result<Option<Handle>, Error> {
        if self.first {
            self.admission.work(1)?;
            self.first = false;
            return Ok(self.frames.first().map(|frame| frame.node.clone()));
        }
        while let Some(frame) = self.frames.last_mut() {
            self.admission.work(1)?;
            let child = frame.node.children.borrow().get(frame.next).cloned();
            let (child, allow_template) = if let Some(child) = child {
                frame.next += 1;
                let allow = match self.templates {
                    Templates::All => true,
                    Templates::None => false,
                    Templates::Active if !frame.shadow_seen => {
                        let active = is_shadow(&child, &self.admission)?;
                        frame.shadow_seen |= active;
                        active
                    }
                    Templates::Active => false,
                };
                (Some(child), allow)
            } else if frame.allow_template && !frame.template_seen {
                frame.template_seen = true;
                let child = match &frame.node.data {
                    NodeData::Element {
                        template_contents, ..
                    } => template_contents.borrow().clone(),
                    _ => None,
                };
                (child, matches!(self.templates, Templates::All))
            } else {
                (None, false)
            };
            if let Some(child) = child {
                self.frames
                    .push(Frame::new(child.clone(), allow_template), &self.admission)?;
                return Ok(Some(child));
            }
            self.frames.pop();
        }
        Ok(None)
    }
}

/// Первый declarative template выбирается без копирования имён и атрибутов.
fn is_shadow(node: &Handle, admission: &Admission) -> Result<bool, Error> {
    let NodeData::Element {
        name,
        attrs,
        template_contents,
        ..
    } = &node.data
    else {
        return Ok(false);
    };
    admission.work(name.local.len())?;
    let local: &str = &name.local;
    if !local.eq_ignore_ascii_case("template") || template_contents.borrow().is_none() {
        return Ok(false);
    }
    for attr in attrs.borrow().iter() {
        admission.work(attr.name.local.len())?;
        if &*attr.name.local == "shadowrootmode" {
            admission.work(attr.value.len())?;
            return Ok(attr.value.eq_ignore_ascii_case("open")
                || attr.value.eq_ignore_ascii_case("closed"));
        }
    }
    Ok(false)
}

impl Iterator for SourceWalk {
    type Item = Handle;
    fn next(&mut self) -> Option<Handle> {
        self.try_next().expect("standalone DOM walk allocation")
    }
}
