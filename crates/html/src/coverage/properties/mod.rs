//! CSS coverage table fragments, concatenated at compile time without changing order.

use crate::coverage::{Prop, Support};
mod box_model;
mod layout;
mod misc;
mod no_op;
mod paint;
mod text;
mod unsupported;

pub(super) const fn m(name: &'static str, sample: &'static str) -> Prop {
    Prop {
        name,
        sample,
        support: Support::Mapped,
    }
}

pub(super) const fn no(name: &'static str, why: &'static str) -> Prop {
    Prop {
        name,
        sample: "",
        support: Support::NoOp(why),
    }
}

pub(super) const fn part(name: &'static str, sample: &'static str, why: &'static str) -> Prop {
    Prop {
        name,
        sample,
        support: Support::Partial(why),
    }
}

pub(super) const fn imp(name: &'static str, why: &'static str) -> Prop {
    Prop {
        name,
        sample: "",
        support: Support::Impossible(why),
    }
}

const fn concatenate<const N: usize>(groups: &[&[Prop]]) -> [Prop; N] {
    let mut out = [const {
        Prop {
            name: "",
            sample: "",
            support: Support::Mapped,
        }
    }; N];
    let mut at = 0;
    let mut group = 0;
    while group < groups.len() {
        let mut i = 0;
        while i < groups[group].len() {
            let p = &groups[group][i];
            out[at] = Prop {
                name: p.name,
                sample: p.sample,
                support: p.support,
            };
            at += 1;
            i += 1;
        }
        group += 1;
    }
    assert!(at == N);
    out
}

/// Полный список свойств, встречающихся в разметке интерфейсов.
///
/// Порядок — по разделам, как в документации.
pub const PROPERTIES: &[Prop] = &concatenate::<205>(&[
    box_model::PROPERTIES,
    layout::PROPERTIES,
    paint::PROPERTIES,
    text::PROPERTIES,
    misc::PROPERTIES,
    no_op::PROPERTIES,
    unsupported::PROPERTIES,
]);
