//! Выход из элемента после детей: псевдоэлементы и закрытие счётчиков.
use super::{
    animation_frames::animation_frames, details_content, finish_element::finish_element,
    pending::Pending,
};
use crate::dom::*;
use crate::style::computed::Display;
use crate::style::select::Ancestor;

pub(super) fn leave(
    mut p: Box<Pending<'_>>,
    path: &[Ancestor],
    counter: &mut u64,
    counters: &mut crate::style::generated::counters::Counters,
    mut children: Vec<Node>,
    out: &mut Vec<Node>,
) {
    let rules = &p.visit.scope.rules;
    let frames = &p.visit.scope.frames;
    let vars = &p.vars;
    let me = &p.me;
    let sibs = p.visit.sibs();
    let vcx = p.visit.vcx;
    if p.hidden {
        children
            .retain(|n| matches!(n, Node::Element(c) if c.style.cascaded().col_role == Some(0)));
        counters.leave();
        *counter += 1;
        out.push(Node::Element(Element {
            marks: Default::default(),
            list_item: None,
            node_id: *counter,
            anim: None,
            inline: false,
            tag: p.tag,
            style: p.style.into(),
            hover: None,
            first_letter: None,
            first_line: None,
            children: children.into(),
            attrs: p.attrs,
        }));
        return;
    }
    if let Some(el) = pseudo_box(
        rules, vars, counters, me, path, sibs, "after", &p.attrs, vcx,
    ) {
        children.push(Node::Element(el));
    }
    let scroll_pseudos = scroll_marker_pass(
        rules,
        vars,
        counters,
        me,
        path,
        sibs,
        &p.tag,
        &p.style,
        &p.attrs,
        &mut children,
        vcx,
    );
    if let Some(el) = pseudo_box(
        rules,
        vars,
        counters,
        me,
        path,
        sibs,
        "scroll-marker",
        &p.attrs,
        vcx,
    ) {
        children.push(Node::Element(el));
    }
    let holds_columns = matches!(p.tag.as_str(), "table" | "colgroup")
        || p.style.col_role == Some(1)
        || matches!(p.style.display, Some(Display::Table | Display::InlineTable));
    if !holds_columns {
        children
            .retain(|n| !matches!(n, Node::Element(c) if c.style.cascaded().col_role.is_some()));
    }
    if let Some(scope) = p.style_scope.take() {
        counters.leave_style_scope(scope);
    }
    if p.box_level {
        counters.leave();
    }
    *counter += 1;
    for kid in &mut children {
        if let Node::Element(c) = kid
            && c.tag.starts_with("::")
        {
            c.style.cascaded_mut().implicit_anchor = Some(*counter);
        }
    }
    if p.tag == "details" {
        children = details_content::wrap(children, &p.attrs, rules, vars, me, path, sibs, vcx);
    }
    let anim = animation_frames(&p.style, frames, vars, p.important.as_ref(), vcx);
    finish_element(
        p.style,
        children,
        p.tag,
        p.attrs,
        anim,
        p.list_item,
        *counter,
        p.hover,
        p.first_letter,
        p.first_line,
        scroll_pseudos,
        out,
        vcx,
    );
}
