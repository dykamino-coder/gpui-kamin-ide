//! Наборы теней по строчным фрагментам: список `text-shadow` наследуется или сбрасывается
//! `none` на каждом `<span>`, а `currentColor` остаётся неразрешённым до фрагмента и берёт его
//! цвет; группы собираются в порядке текста без повторов.

use super::atomic;
use crate::dom::{Element, Node};
use crate::style::computed::{Computed, Shadow};
use crate::style::values::value::Color;

pub(super) struct ShadowStyle {
    color: Color,
    list: Vec<Shadow>,
}

impl ShadowStyle {
    pub(super) fn new(style: &Computed) -> Self {
        Self {
            color: style.color.unwrap_or(Color {
                r: 0.,
                g: 0.,
                b: 0.,
                a: 1.,
            }),
            list: raw_list(style),
        }
    }

    pub(super) fn child(&self, e: &Element) -> Self {
        Self {
            color: e.style.cascaded().color.unwrap_or(self.color),
            list: if e.style.used().text_shadow_none {
                Vec::new()
            } else if e.style.cascaded().text_shadow.is_some() {
                raw_list(&e.style.used())
            } else {
                self.list.clone()
            },
        }
    }

    pub(super) fn resolved(&self) -> Vec<Shadow> {
        self.list
            .iter()
            .cloned()
            .map(|mut shadow| {
                shadow.color = shadow.used_color(self.color).into();
                shadow
            })
            .collect()
    }
}

fn raw_list(style: &Computed) -> Vec<Shadow> {
    style
        .text_shadow
        .clone()
        .into_iter()
        .chain(style.text_shadow_rest.iter().cloned())
        .collect()
}

pub(super) fn collect_groups(
    nodes: &[Node],
    current: &ShadowStyle,
    out: &mut Vec<Vec<Shadow>>,
    vcx: crate::document::state::ValueCx<'_>,
) {
    let resolved = current.resolved();
    for node in nodes {
        match node {
            Node::Text(text) => {
                if !resolved.is_empty() && !text.trim().is_empty() && !out.contains(&resolved) {
                    out.push(resolved.clone());
                }
            }
            Node::Element(e) if !atomic(e, vcx) => {
                collect_groups(&e.children, &current.child(e), out, vcx)
            }
            Node::Element(_) => {}
        }
    }
}
