//! Возобновляемая подстановка: выбранный fallback на heap, запрос имени — вызывающему.

use super::{Call, Unsubstituted, Work, calls, custom_name};
use crate::budget::admission::Buffer;

struct Frame {
    raw: String,
    calls: std::vec::IntoIter<Call>,
    from: usize,
    result: String,
}

pub(crate) enum Step {
    Need(String),
    Done(String),
}

pub(crate) struct Expansion {
    frames: Buffer<Frame>,
    waiting: Option<Call>,
    literal: Option<String>,
}

impl Expansion {
    pub(super) fn new(
        raw: String,
        calls: Vec<Call>,
        work: &Work<'_>,
    ) -> Result<Self, Unsubstituted> {
        work.spend(raw.len().saturating_mul(8))?;
        let mut frames = Buffer::default();
        work.admit(frames.push(
            Frame {
                raw,
                calls: calls.into_iter(),
                from: 0,
                result: String::new(),
            },
            work.admission(),
        ))?;
        Ok(Self {
            frames,
            waiting: None,
            literal: None,
        })
    }

    pub(crate) fn start(raw: &str, work: &Work<'_>) -> Result<Self, Unsubstituted> {
        let calls = calls(raw);
        if calls.is_empty() {
            work.fit(0, raw.len())?;
            return Ok(Self {
                frames: Buffer::default(),
                waiting: None,
                literal: Some(raw.to_string()),
            });
        }
        Self::new(raw.to_string(), calls, work)
    }

    pub(crate) fn step(&mut self, work: &Work<'_>) -> Result<Step, Unsubstituted> {
        if let Some(value) = self.literal.take() {
            return Ok(Step::Done(value));
        }
        loop {
            let frame = self.frames.last_mut().expect("active substitution frame");
            if let Some(call) = frame.calls.next() {
                work.append(&mut frame.result, &frame.raw[frame.from..call.start])?;
                frame.from = call.end;
                let name = custom_name(&call.name).then(|| crate::style::css::unescape(&call.name));
                self.waiting = Some(call);
                if let Some(name) = name {
                    return Ok(Step::Need(name));
                }
                self.supply(None, work)?;
            } else {
                work.append(&mut frame.result, &frame.raw[frame.from..])?;
                let frame = self.frames.pop().expect("completed substitution frame");
                if self.frames.is_empty() {
                    return Ok(Step::Done(frame.result));
                }
                self.append_replacement(&frame.result, work)?;
            }
        }
    }

    pub(crate) fn supply(
        &mut self,
        value: Option<String>,
        work: &Work<'_>,
    ) -> Result<(), Unsubstituted> {
        let call = self.waiting.take().expect("requested variable");
        if let Some(value) = value {
            return self.append_replacement(&value, work);
        }
        let raw = call.fallback.ok_or(Unsubstituted::Invalid)?;
        work.spend(raw.len().saturating_mul(8))?;
        let calls = calls(&raw).into_iter();
        work.admit(self.frames.push(
            Frame {
                raw,
                calls,
                from: 0,
                result: String::new(),
            },
            work.admission(),
        ))?;
        Ok(())
    }

    fn append_replacement(&mut self, value: &str, work: &Work<'_>) -> Result<(), Unsubstituted> {
        let out = &mut self.frames.last_mut().expect("substitution parent").result;
        work.append(out, " ")?;
        work.append(out, value)?;
        work.append(out, " ")
    }
}
