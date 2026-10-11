//! Segment state for ruby; split out to keep the owning module within 250 lines.

use super::ruby_role;
use super::{RubySegment, RubyUnit};
use crate::dom::Node;
use crate::text::text_box::blank_text;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Kind {
    Text,
    Rb,
    Rbc,
    Rt,
    Rtc,
    Blank,
    Drop,
}

pub(super) fn fresh() -> RubySegment {
    RubySegment {
        bases: Vec::new(),
        levels: Vec::new(),
    }
}

pub(super) fn flush_run(run: &mut RubyUnit, cur: &mut RubySegment) {
    if !run.is_empty() {
        cur.bases.push(std::mem::take(run));
    }
}

pub(super) fn close_segment(cur: &mut RubySegment, out: &mut Vec<RubySegment>) {
    if !cur.bases.is_empty() || !cur.levels.is_empty() {
        out.push(std::mem::replace(cur, fresh()));
    }
}

// База после аннотаций — новый сегмент (§2.3.1). Явный `<rbc>` — свой
// контейнер баз, а сегмент — ОДИН контейнер баз с аннотациями за ним
// (§2.3.1): база после `<rbc>` его не продолжает, даже без аннотаций.
// Прежде `<rbc>e</rbc><rbc>f</rbc><rbc>g</rbc><rtc>h</rtc>` склеивались
// в один сегмент, и `h` вставала над `e`, а не над `g`
// (`ruby-box-generation-001-ref`).
pub(super) fn base_starts(
    cur: &mut RubySegment,
    out: &mut Vec<RubySegment>,
    loose_level: &mut bool,
    sealed: &mut bool,
) {
    if !cur.levels.is_empty() || *sealed {
        close_segment(cur, out);
        *loose_level = false;
    }
    *sealed = false;
}

pub(super) fn kind_of(n: &Node) -> Kind {
    match n {
        Node::Text(t) if blank_text(t) => Kind::Blank,
        Node::Text(_) => Kind::Text,
        Node::Element(k) if k.tag == "rp" => Kind::Drop,
        Node::Element(k) => match ruby_role(k) {
            Some(crate::style::computed::RubyRole::Base) => Kind::Rb,
            Some(crate::style::computed::RubyRole::BaseContainer) => Kind::Rbc,
            Some(crate::style::computed::RubyRole::Text) => Kind::Rt,
            Some(crate::style::computed::RubyRole::TextContainer) => Kind::Rtc,
            _ => Kind::Text,
        },
    }
}

pub(super) fn collect_bases(k: &crate::dom::Element, cur: &mut RubySegment) {
    let kids: Vec<Kind> = k.children.iter().map(kind_of).collect();
    let mut inner: RubyUnit = Vec::new();
    for (j, c) in k.children.iter().enumerate() {
        match kids[j] {
            Kind::Rb => {
                if !inner.is_empty() {
                    cur.bases.push(std::mem::take(&mut inner));
                }
                cur.bases.push(vec![c.clone()]);
            }
            Kind::Blank => {
                let prev = kids[..j].iter().rev().copied().find(|k| *k != Kind::Blank);
                let next = kids[j + 1..].iter().copied().find(|k| *k != Kind::Blank);
                match (prev, next) {
                    (Some(Kind::Rb), Some(Kind::Rb)) => cur.bases.push(vec![c.clone()]),
                    (Some(Kind::Text), Some(_)) | (Some(_), Some(Kind::Text)) => {
                        inner.push(c.clone())
                    }
                    _ => {}
                }
            }
            Kind::Drop => {}
            _ => inner.push(c.clone()),
        }
    }
    if !inner.is_empty() {
        cur.bases.push(inner);
    }
}
