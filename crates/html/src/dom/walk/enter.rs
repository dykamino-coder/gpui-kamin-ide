//! Вход в элемент: каскад и счётчики до обхода детей.
use super::{
    ancestor::{element_ancestor, element_attrs},
    cascade::cascade_element,
    pseudo_styles::pseudo_styles,
    style_normalization::normalize_style,
};
use super::{content, language};
use super::{pending::Pending, sibling_survey::Visit};
use crate::dom::tree_index::TreeIndex;
use crate::dom::*;
use crate::style::computed::Display;
use crate::style::select::Ancestor;
use markup5ever_rcdom::NodeData;
use std::rc::Rc;

#[allow(clippy::too_many_arguments)]
pub(super) fn enter<'a>(
    visit: Visit<'a>,
    tree: &TreeIndex,
    _counter: &mut u64,
    counters: &mut crate::style::generated::counters::Counters,
    path: &[Ancestor],
    out: &mut Vec<Node>,
) -> Option<Box<Pending<'a>>> {
    let handle = visit.handle();
    let NodeData::Element { name, attrs, .. } = &handle.data else {
        return None;
    };
    let spot = visit.survey.spots[visit.index];
    let sibs = visit.sibs();
    let rules = &visit.scope.rules;
    let vars = &visit.vars;
    let vcx = visit.vcx;
    let tag = local_name(&name.local);
    if DROP_TAGS.contains(&tag.as_str()) {
        return None;
    }
    let attrs = element_attrs(&attrs.borrow());
    let mut me = element_ancestor(
        handle,
        &tag,
        &attrs,
        content::html_attributes(&name.ns, tree.xhtml),
        spot,
        sibs,
        tree,
    );
    let (mut style, own_vars, scheme, shadow, important) = cascade_element(
        &mut me, &attrs, &tag, handle, tree, rules, vars, path, spot, sibs, vcx,
    );
    let vcx = vcx.with_scheme(scheme);
    let vars = &own_vars;
    normalize_style(&mut style, &tag, &attrs, path, vcx);
    content::snapshot_generated(&mut me, &style, path.last());
    // Правила с `:hover` собираются отдельным слоем: в базовый стиль
    // им нельзя, иначе элемент выглядел бы всегда наведённым.
    let (hover, first_letter, first_line, marker_layer) =
        pseudo_styles(&mut style, &tag, rules, vars, &me, path, sibs, vcx);

    let reversed_start = |nm: &str, _counters: &mut crate::style::generated::counters::Counters| {
        crate::style::generated::counters_scan::reversed_initial(
            tree,
            rules,
            vars,
            nm,
            handle,
            &me,
            path,
            sibs,
            &visit.survey.nodes,
            &visit.survey.spots,
            visit.index,
            vcx,
        )
    };
    let hidden = style.display == Some(Display::None);
    if hidden && tag == "html" {
        out.push(Node::Element(Element {
            marks: Default::default(),
            tag,
            inline: false,
            node_id: 0,
            style: style.into(),
            hover: None,
            first_letter: None,
            first_line: None,
            children: (vec![]).into(),
            attrs: vec![],
            anim: None,
            list_item: None,
        }));
        return None;
    }
    if hidden && style.col_role.is_none() {
        return None;
    }
    let box_level = style.display != Some(Display::Contents);
    if box_level {
        counters.enter();
        if !hidden {
            counters.set_quote_language(language::parent(&me, path).unwrap_or(""));
            if let Some(q) = &style.quotes {
                counters.set_quotes(q.clone());
            }
        }
    }
    let mut is_list_item = false;
    apply_counter_decls(
        &style,
        counters,
        &tag,
        &attrs,
        &mut is_list_item,
        &reversed_start,
    );
    let list_item = (!hidden && is_list_item).then(|| counters.value_of("list-item"));
    let style_scope = (!hidden && box_level && style.contain_style == Some(true))
        .then(|| counters.enter_style_scope());
    let mut children = vec![];
    if !hidden {
        super::marker::apply(
            &mut style,
            marker_layer,
            is_list_item,
            &me,
            path,
            &attrs,
            counters,
            vcx,
        );
        if let Some(el) = pseudo_box(
            rules, vars, counters, &me, path, sibs, "before", &attrs, vcx,
        ) {
            children.push(Node::Element(el));
        }
    }
    let mut visit = visit;
    visit.vcx = vcx;
    Some(Box::new(Pending {
        visit,
        me,
        tag,
        attrs,
        style,
        vars: Rc::new(own_vars),
        shadow,
        important,
        hover,
        first_letter,
        first_line,
        list_item,
        box_level,
        style_scope,
        hidden,
        children,
    }))
}
