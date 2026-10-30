//! Подстановка типизированных атрибутов: heap-стек и циклические substitution contexts.

use super::{CascadeCx, ValueCx, attr_cast::attr_cast};
use crate::budget::admission::Buffer;
use crate::css_syntax::{escape_end, functions, unescape};
use crate::style::css::variable_values::{Unsubstituted, Work};
use crate::style::css::{block_end, split_top_level, strip_comments};

struct Call {
    start: usize,
    end: usize,
    name: String,
    ty: String,
    fallback: Option<String>,
}
struct Frame {
    raw: String,
    calls: std::vec::IntoIter<Call>,
    from: usize,
    out: String,
    context: Option<String>,
    cyclic: bool,
    waiting: Option<(Call, bool)>,
    failed: Option<Unsubstituted>,
}
impl Frame {
    fn new(raw: &str, context: Option<String>, work: &Work<'_>) -> Result<Self, Unsubstituted> {
        work.spend(raw.len().saturating_mul(8))?;
        let mut calls = Vec::new();
        let mut from = 0;
        for f in functions(raw) {
            if f.start < from || !f.name.eq_ignore_ascii_case("attr") {
                continue;
            }
            let end = block_end(raw, f.open, '(').ok_or(Unsubstituted::Invalid)?;
            let inner = &raw[f.open + 1..end];
            let args = split_top_level(inner, ',');
            let head = strip_comments(args[0]);
            let head = head.trim();
            let mut at = 0;
            while let Some(c) = head[at..].chars().next() {
                if c == '\\' {
                    at = escape_end(head, at);
                } else if c.is_whitespace() {
                    break;
                } else {
                    at += c.len_utf8();
                }
            }
            from = end + 1;
            calls.push(Call {
                start: f.start,
                end: from,
                name: unescape(&head[..at]),
                ty: head[at..].trim().into(),
                fallback: (args.len() > 1).then(|| inner[args[0].len() + 1..].into()),
            });
        }
        Ok(Self {
            raw: raw.into(),
            calls: calls.into_iter(),
            from: 0,
            out: String::new(),
            context,
            cyclic: false,
            waiting: None,
            failed: None,
        })
    }
}

fn attribute<'a>(name: &str, cx: CascadeCx<'a>) -> Option<(&'a str, &'a str)> {
    let local = match name.split_once('|') {
        Some(("", local)) => local,
        Some(_) => return None,
        None => name,
    };
    cx.attrs
        .iter()
        .find(|(key, _)| {
            if cx.html_attrs {
                key.eq_ignore_ascii_case(local)
            } else {
                key == local
            }
        })
        .map(|(key, value)| (key.as_str(), value.as_str()))
}

fn replacement(frame: &mut Frame, value: &str, work: &Work<'_>) -> Result<(), Unsubstituted> {
    // Attr-tainted URL делает объявление IACVT после выбора результата, без fallback.
    if functions(value).any(|f| f.name.eq_ignore_ascii_case("url")) {
        return Err(Unsubstituted::Invalid);
    }
    work.append(&mut frame.out, " ")?;
    work.append(&mut frame.out, value)?;
    work.append(&mut frame.out, " ")
}

fn receive(
    frames: &mut Buffer<Frame>,
    value: Result<String, Unsubstituted>,
    vcx: ValueCx<'_>,
    work: &Work<'_>,
) -> Result<(), Unsubstituted> {
    let parent = frames.last_mut().expect("attribute parent");
    let (call, cast) = parent.waiting.take().expect("attribute continuation");
    let value = match value {
        Err(error @ Unsubstituted::Limit(_)) => return Err(error),
        value if cast => value
            .ok()
            .and_then(|value| attr_cast(&value, &call.ty, vcx)),
        value => {
            parent.failed = value.as_ref().err().copied();
            value.ok()
        }
    };
    if let Some(value) = value {
        parent.failed = replacement(parent, &value, work).err();
    } else if cast {
        if let Some(fallback) = &call.fallback {
            let child = Frame::new(fallback, None, work)?;
            parent.waiting = Some((call, false));
            work.admit(frames.push(child, work.admission()))?;
        } else {
            parent.failed = Some(Unsubstituted::Invalid);
        }
    }
    Ok(())
}

fn expand(
    value: &str,
    cx: CascadeCx<'_>,
    vcx: ValueCx<'_>,
    work: &Work<'_>,
) -> Result<String, Unsubstituted> {
    let mut frames = Buffer::default();
    work.admit(frames.push(Frame::new(value, None, work)?, work.admission()))?;
    loop {
        let frame = frames.last_mut().expect("attribute frame");
        if frame.failed.is_none()
            && let Some(call) = frame.calls.next()
        {
            work.append(&mut frame.out, &frame.raw[frame.from..call.start])?;
            frame.from = call.end;
            if call.name.starts_with("*|") {
                frame.failed = Some(Unsubstituted::Invalid);
                continue;
            }
            // Raw strings остаются generated-content вычислителю владельца элемента.
            if call.ty.is_empty() || call.ty.eq_ignore_ascii_case("raw-string") {
                work.append(&mut frame.out, &frame.raw[call.start..call.end])?;
                continue;
            }
            let attr = attribute(&call.name, cx);
            frame.waiting = Some((call, true));
            if let Some((name, raw)) = attr {
                if let Some(at) = frames
                    .iter()
                    .position(|f| f.context.as_deref() == Some(name))
                {
                    for frame in &mut frames[at..] {
                        frame.cyclic = true;
                    }
                } else {
                    work.admit(
                        frames.push(Frame::new(raw, Some(name.into()), work)?, work.admission()),
                    )?;
                    continue;
                }
            }
            receive(&mut frames, Err(Unsubstituted::Invalid), vcx, work)?;
            continue;
        }
        let mut frame = frames.pop().expect("completed attribute frame");
        let result = if let Some(error) = frame.failed {
            Err(error)
        } else if frame.cyclic {
            Err(Unsubstituted::Invalid)
        } else {
            work.append(&mut frame.out, &frame.raw[frame.from..])?;
            Ok(frame.out)
        };
        if frames.is_empty() {
            return result;
        }
        receive(&mut frames, result, vcx, work)?;
    }
}

pub(in crate::style::cascade) fn resolve_attrs(
    _key: &str,
    value: &str,
    cx: CascadeCx<'_>,
    vcx: ValueCx<'_>,
) -> String {
    if !functions(value).any(|f| f.name.eq_ignore_ascii_case("attr")) {
        return value.into();
    }
    let revision = vcx.doc.revision_budget();
    let work = Work::new(&revision);
    let value = expand(value, cx, vcx, &work).unwrap_or_else(|_| "unset".into());
    work.finish(vcx.doc);
    value
}
