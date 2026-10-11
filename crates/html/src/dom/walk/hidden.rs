//! Hidden for walk; split out to keep the owning module within 250 lines.

use super::walk_children;
use crate::dom::*;
use crate::style::computed::Computed;
use crate::style::css::{Decls, Keyframes, Rule};
use crate::style::select::Ancestor;
use markup5ever_rcdom::Handle;
use std::collections::HashMap;

#[allow(clippy::too_many_arguments)]
#[allow(clippy::needless_return)]
pub(super) fn hidden_element(
    style: Computed,
    tag: String,
    attrs: Vec<(String, String)>,
    me: &Ancestor,
    handle: &Handle,
    rules: &[Rule],
    vars: &Decls,
    frames: &HashMap<String, Keyframes>,
    counter: &mut u64,
    counters: &mut crate::style::generated::counters::Counters,
    path: &[Ancestor],
    preserve: bool,
    reversed_start: &dyn Fn(&str, &mut crate::style::generated::counters::Counters) -> i32,
    out: &mut Vec<Node>,
) {
    // Table columns use an internal non-flow display, but still
    // generate boxes (CSS 2.1 §17.2). Their counter directives
    // apply; actual display:none nodes have no counters (§12.4.3).
    // CSS 2.1 §14.2: the root background paints the canvas even
    // without a root box. Keep its style for canvas propagation.
    if me.tag == "html" {
        out.push(Node::Element(Element {
            tag: "html".to_string(),
            inline: false,
            node_id: 0,
            style,
            hover: None,
            first_letter: None,
            first_line: None,
            children: vec![],
            attrs: vec![],
            anim: Default::default(),
            list_item: None,
        }));
        return;
    }
    let Some(role) = style.col_role else {
        return;
    };
    counters.enter();
    apply_counter_decls(&style, counters, &tag, &attrs, &mut false, &reversed_start);
    // §17.2.1: у колонки детей нет вовсе, у группы колонок
    // остаются только колонки.
    let mut kids: Vec<Node> = vec![];
    if role == 1 {
        let mut path2 = path.to_vec();
        path2.push(me.clone());
        let mut raw: Vec<Node> = vec![];
        walk_children(
            handle,
            rules,
            vars,
            frames,
            counter,
            counters,
            &path2,
            style.preserve_newlines.unwrap_or(preserve),
            &mut raw,
        );
        kids = raw
            .into_iter()
            .filter(|n| matches!(n, Node::Element(c) if c.style.col_role == Some(0)))
            .collect();
    }
    counters.leave();
    *counter += 1;
    out.push(Node::Element(Element {
        list_item: None,
        node_id: *counter,
        anim: None,
        inline: false,
        tag,
        style,
        hover: None,
        first_letter: None,
        first_line: None,
        children: kids,
        attrs,
    }));
    return;
}
