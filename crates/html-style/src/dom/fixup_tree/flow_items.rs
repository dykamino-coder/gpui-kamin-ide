//! Правки потока по собранному дереву: вбегание `run-in` и флоаты у детей
//! flex/grid.
//!
//! `fold_run_ins` (CSS 2.1 §9.2.3): элемент `display: run-in` без блочного
//! содержимого, за которым сквозь схлопываемый пробел идёт обычный блок,
//! становится его первым строчным ребёнком; внепоточные соседи прозрачны.
//! `flex_items_lose_float` (css-flexbox-1 §4, css-grid-2 §6): у потокового
//! ребёнка flex/grid `float` и `clear` вычисляются в `none`.

use crate::budget::admission::{Admission, Error};
use crate::dom::normalization::{Transform, apply};
use crate::dom::*;
use crate::style::computed::{Computed, Display};

/// Вбегание `display: run-in` (CSS 2.1 §9.2.3): элемент без блочного
/// содержимого, за которым (сквозь пробельный текст) идёт обычная блочная
/// коробка, становится её ПЕРВЫМ СТРОЧНЫМ ребёнком; во всех остальных
/// случаях он ведёт себя как блок (это уже так — разбор дал Block).
pub fn fold_run_ins(
    admission: &Admission,
    nodes: Vec<Node>,
    parent: Option<&Computed>,
) -> Result<Vec<Node>, Error> {
    apply(
        &RunIn,
        nodes,
        parent.is_some_and(|p| p.inh.text_flow.keep_spaces == Some(true)),
        admission,
    )
}

pub(crate) struct RunIn;

impl Transform for RunIn {
    type Context = bool;

    fn enter(&self, el: &mut Element, _: &bool, _: &Admission) -> Result<bool, Error> {
        Ok(el.style.cascaded().inh.text_flow.keep_spaces == Some(true))
    }

    fn after_child(
        &self,
        nodes: &mut Children,
        index: usize,
        keep: &bool,
        admission: &Admission,
    ) -> Result<usize, Error> {
        fold_at(admission, nodes, index, *keep)
    }
}

fn fold_at(
    admission: &crate::budget::admission::Admission,
    nodes: &mut Children,
    i: usize,
    keep: bool,
) -> Result<usize, crate::budget::admission::Error> {
    // Any in-flow block, including behind inline wrappers, prevents folding.
    let runs_in = match &nodes[i] {
        Node::Element(e) => {
            e.style.cascaded().reset_context.run_in == Some(true)
                && !crate::style::flow::has_float_or_absolute_position(e.style.cascaded())
                && !holds_block(admission, &e.children)?
        }
        _ => false,
    };
    if !runs_in {
        return Ok(i + 1);
    }
    // Only the next ordinary block qualifies; whitespace and out-of-flow
    // siblings are transparent (CSS 2.1 section 9.2.3).
    let Some(j) = (i + 1..nodes.len()).find(|&j| {
            !matches!(&nodes[j], Node::Text(t) if t.is_empty() || (!keep && crate::dom::whitespace::blank_text(t))) && !matches!(&nodes[j], Node::Element(t) if crate::style::flow::has_float_or_absolute_position(t.style.cascaded()))
        }) else {
            return Ok(i + 1);
        };
    let target_ok = matches!(&nodes[j], Node::Element(t)
    if !t.inline
        && t.style.cascaded().reset_context.run_in != Some(true)
        && !matches!(
            t.style.cascaded().reset_box_layout.display,
            Some(Display::None)
                | Some(Display::InlineBlock)
                | Some(Display::InlineFlex)
                | Some(Display::InlineGrid)
                | Some(Display::InlineTable)
                | Some(Display::Table)
                | Some(Display::TableRow)
                | Some(Display::TableRowGroup)
                | Some(Display::TableCell)
        ));
    if !target_ok {
        return Ok(i + 1);
    }
    // Reserve both edited lists before moving the run-in. Shared snapshots
    // remain immutable, including when either reservation rejects this stage.
    let nodes = nodes.edit(admission, 0)?;
    let Node::Element(target) = &mut nodes[j] else {
        unreachable!("validated target")
    };
    target.children.edit(admission, 1)?;
    let Node::Element(mut run) = nodes.remove(i) else {
        unreachable!("validated run-in")
    };
    run.inline = true;
    run.style.cascaded_mut().reset_box_layout.display = None;
    run.style.cascaded_mut().reset_context.run_in = None;
    // Preserve the original inheritance behavior: baking in the old parent's
    // style would also merge render properties and change run-in inheritance.
    let Node::Element(target) = &mut nodes[j - 1] else {
        unreachable!("target shifted after removal")
    };
    target.children.insert(0, Node::Element(run));
    // The shifted sibling must enter with its newly inserted subtree.

    Ok(i)
}

fn holds_block(
    admission: &crate::budget::admission::Admission,
    nodes: &[Node],
) -> Result<bool, crate::budget::admission::Error> {
    let transparent = |ch: &Element| {
        ch.inline
            || matches!(
                ch.style.cascaded().reset_box_layout.display,
                Some(
                    Display::InlineBlock
                        | Display::InlineFlex
                        | Display::InlineGrid
                        | Display::InlineTable
                )
            )
    };
    let visible = |ch: &Element| {
        !crate::style::flow::has_float_or_absolute_position(ch.style.cascaded())
            && ch.style.cascaded().reset_box_layout.display != Some(Display::None)
    };
    for node in crate::dom::traversal::nodes(admission, nodes, |ch| visible(ch) && transparent(ch))
    {
        if matches!(node?, Node::Element(ch) if visible(ch) && !transparent(ch)) {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn flex_items_lose_float(admission: &Admission, nodes: Vec<Node>) -> Result<Vec<Node>, Error> {
    apply(&FlexItemFloat, nodes, (), admission)
}

pub(crate) struct FlexItemFloat;

impl Transform for FlexItemFloat {
    type Context = ();

    fn enter(&self, _: &mut Element, _: &(), _: &Admission) -> Result<(), Error> {
        Ok(())
    }

    fn leave(&self, el: &mut Element, _: &(), _: &Admission) -> Result<(), Error> {
        if !matches!(
            el.style.cascaded().reset_box_layout.display,
            Some(Display::Grid)
                | Some(Display::InlineGrid)
                | Some(Display::Flex)
                | Some(Display::InlineFlex)
        ) {
            return Ok(());
        }
        for child in el.children.iter_mut() {
            let Node::Element(child) = child else {
                continue;
            };
            if crate::style::flow::absolutely_positioned(
                child.style.cascaded().reset_positioned.position,
            ) {
                continue;
            }
            child.style.cascaded_mut().reset_box_layout.float = None;
            child.style.cascaded_mut().reset_box_layout.clear = None;
        }
        Ok(())
    }
}
