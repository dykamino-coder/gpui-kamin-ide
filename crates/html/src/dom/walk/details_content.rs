//! HTML §15.5.4 (the details and summary elements): коробка-слот
//! `::details-content` у `<details>`. Теневое дерево спеки — `<slot summary>`,
//! затем `<slot ::details-content>`; Blink (`HTMLDetailsElement::
//! DidAddUserAgentShadowRoot`) и Gecko строят это UA-дерево, Servo прячет закрытое
//! содержимое через `display: none` на псевдо. Здесь слот — синтетический дочерний
//! `::details-content`: первым идёт первый `summary`, все прочие дети (кроме
//! `::before`/`::after` хозяина) — в слот. UA-стиль слота `display: block`,
//! закрытого — `content-visibility: hidden`; авторские правила каскадируют поверх.

use crate::document::state::ValueCx;
use crate::dom::*;
use crate::style::computed::Computed;
use crate::style::css::{Decls, Rule, Selector, parse_decls};
use crate::style::select::matching::matches_ignoring_pseudo;
use crate::style::select::{Ancestor, Sibs};

fn ua_rule() -> Option<Rule> {
    let css = "display: block";
    Some(Rule {
        sel: Selector::parse("*")?,
        decls: parse_decls(css),
        order: 0,
        origin: 0,
        layer: Vec::new(),
    })
}

/// Rebuild the children of a `<details>` box: summary, then the slot box.
#[allow(clippy::too_many_arguments)]
pub(super) fn wrap(
    children: Vec<Node>,
    attrs: &[(String, String)],
    rules: &[Rule],
    vars: &Decls,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
    vcx: ValueCx<'_>,
) -> Vec<Node> {
    let open = attrs.iter().any(|(k, _)| k == "open");
    let Some(ua) = ua_rule() else {
        return children;
    };
    let mut matched: Vec<&Rule> = std::iter::once(&ua)
        .chain(
            rules
                .iter()
                .filter(|r| r.sel.pseudo.as_deref() == Some("details-content"))
                .filter(|r| matches_ignoring_pseudo(&r.sel, me, path, sibs)),
        )
        .collect();
    // The UA `content-visibility: hidden` of a closed slot loses to any
    // author value (`details-pseudo-elements-002`: `visible` shows it).
    let authored = matched
        .iter()
        .any(|r| r.decls.contains_key("content-visibility"));
    let mut style = Computed::resolve_with_vars(&mut matched, &Decls::new(), vars, vcx);
    if !open && !authored {
        style.apply_one("content-visibility", "hidden", vcx);
    }
    // An in-flow block slot is an ordinary block box for fragmentation
    // (`style_normalization`): a forced break on its first child propagates.
    {
        use crate::style::computed::Display;
        style.plain_block_box = matches!(
            style.display,
            Some(Display::Block | Display::ListItem | Display::Flex | Display::Grid)
        ) && style.float.is_none_or(|f| f == 0);
    }
    let is_pseudo = |n: &Node| matches!(n, Node::Element(e) if e.tag.starts_with("::"));
    let lead = children.iter().take_while(|n| is_pseudo(n)).count();
    let trail = children[lead..]
        .iter()
        .rev()
        .take_while(|n| is_pseudo(n))
        .count();
    let mut children = children;
    let tail: Vec<Node> = children.split_off(children.len() - trail);
    let body: Vec<Node> = children.split_off(lead);
    let mut out = children;
    let mut slot = Vec::with_capacity(body.len());
    let mut summary = None;
    for node in body {
        match node {
            Node::Element(e) if summary.is_none() && e.tag == "summary" => summary = Some(e),
            other => slot.push(other),
        }
    }
    if let Some(s) = summary {
        out.push(Node::Element(s));
    }
    out.push(Node::Element(Element {
        marks: Default::default(),
        list_item: None,
        node_id: 0,
        anim: None,
        tag: "::details-content".into(),
        style: style.into(),
        hover: None,
        first_letter: None,
        first_line: None,
        children: slot,
        attrs: vec![],
        inline: false,
    }));
    out.extend(tail);
    out
}
