//! Вход в элемент: каскад и счётчики до обхода детей.
use super::element_sharing::{ElementSharing, Key, Shared};
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
    sharing: &ElementSharing,
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
        path,
    );
    me.ancestor_bloom = Some(
        path.last()
            .and_then(|a| a.ancestor_bloom)
            .unwrap_or_else(|| {
                path.iter().fold(
                    Default::default(),
                    |b: crate::style::select::bloom::Bloom, a| b.with_node(a),
                )
            })
            .with_node(&me),
    );
    let excluded = tree.shadow_of(handle).is_some() || !tree.slotted_rules_of(handle).is_empty();
    let key = Key::new(
        visit.parent_share,
        &visit.scope,
        vars,
        vcx,
        &me,
        &tag,
        &attrs,
        excluded,
    );
    let found = key
        .as_ref()
        .and_then(|key| sharing.lookup(key, &me, path, sibs, vcx));
    let (shared, shadow) = match found {
        Some(shared) => (shared, None),
        None => {
            let observed = vcx.doc.diagnostic_observations();
            let candidates = Rc::new(visit.scope.index.candidates(&me, path));
            let (mut style, own_vars, scheme, shadow, important) = cascade_element(
                &mut me,
                &attrs,
                &sharing.inline.block(&attrs, vcx),
                &tag,
                handle,
                tree,
                &visit.scope,
                &candidates,
                vars,
                path,
                spot,
                sibs,
                vcx,
            );
            let vcx = vcx.with_scheme(scheme);
            normalize_style(&mut style, &tag, &attrs, path, vcx);
            let layers = pseudo_styles(
                &mut style,
                &tag,
                &visit.scope,
                &candidates,
                &own_vars,
                &me,
                path,
                sibs,
                vcx,
            );
            let shared = Shared {
                candidates,
                style,
                vars: Rc::new(own_vars),
                scheme,
                important,
                layers,
                share: sharing.fresh(),
            };
            if let Some(key) = &key
                && shadow.is_none()
                && observed == vcx.doc.diagnostic_observations()
            {
                sharing.insert(key, &me, path, sibs, &shared, vcx);
            }
            (shared, shadow)
        }
    };
    let Shared {
        candidates,
        mut style,
        vars: own_vars,
        scheme,
        important,
        layers: (hover, first_letter, first_line, marker_layer),
        share,
    } = shared;
    let vcx = vcx.with_scheme(scheme);
    let vars = &own_vars;
    content::snapshot_generated(&mut me, &style, path.last());

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
    let hidden = style.reset_box_layout.display == Some(Display::None);
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
    if hidden && style.reset_context.col_role.is_none() {
        return None;
    }
    let box_level = style.reset_box_layout.display != Some(Display::Contents);
    if box_level {
        counters.enter();
        if !hidden {
            counters.set_quote_language(&language::parent(&me, path));
            if let Some(q) = &style.reset_generated.quotes {
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
    let style_scope = (!hidden && box_level && style.reset_context.contain_style == Some(true))
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
            &visit.scope,
            &candidates,
            vars,
            counters,
            &me,
            path,
            sibs,
            "before",
            &attrs,
            vcx,
        ) {
            children.push(Node::Element(el));
        }
    }
    let mut visit = visit;
    visit.vcx = vcx;
    Some(Box::new(Pending {
        visit,
        candidates,
        me,
        tag,
        attrs,
        style,
        vars: own_vars,
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
        share,
    }))
}
