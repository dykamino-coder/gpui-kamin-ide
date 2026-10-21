//! Стиль узла и его псевдоэлемента при упреждающем подсчёте счётчиков.

use crate::document::state::ValueCx;
use crate::style::computed::{Computed, Display};
use crate::style::css::{DeclBlock, Rule, VarMap};
use crate::style::select::matching::matches_ignoring_pseudo;
use crate::style::select::{Ancestor, Sibs};
use markup5ever_rcdom::{Handle, NodeData};

/// Resolve counter directives and display with the rendering cascade.
pub(super) fn scan_style(
    h: &Handle,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
    rules: &[Rule],
    vars: &VarMap,
    vcx: ValueCx<'_>,
) -> Option<Computed> {
    let NodeData::Element { attrs, .. } = &h.data else {
        return None;
    };
    let inline_decls: DeclBlock = attrs
        .borrow()
        .iter()
        .find(|a| &*a.name.local == "style")
        .map(|a| crate::style::css::parse_decls_in(&a.value, vcx))
        .unwrap_or_default();
    let matched: Vec<&Rule> = rules
        .iter()
        .filter(|r| crate::style::select::matching::matches(&r.sel, me, path, sibs))
        .collect();
    let mut style = Computed::resolve_with_vars(&matched, &inline_decls, vars, vcx);
    crate::dom::content::inherit_generated(&mut style, path.last());
    crate::dom::apply_value_hint(&mut style, me);
    Some(style)
}

/// Стиль псевдоэлемента, если правила дают ему коробку.
pub(super) fn pseudo_style(
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
    which: &str,
    rules: &[Rule],
    vars: &VarMap,
    vcx: ValueCx<'_>,
) -> Option<Computed> {
    let matched: Vec<&Rule> = rules
        .iter()
        .filter(|r| r.sel.pseudo.as_deref() == Some(which))
        .filter(|r| matches_ignoring_pseudo(&r.sel, me, path, sibs))
        .collect();
    if matched.is_empty() {
        return None;
    }
    let mut style = Computed::resolve_with_vars(&matched, &DeclBlock::new(), vars, vcx);
    crate::dom::content::inherit_generated(&mut style, Some(me));
    // Без содержимого коробки нет, а значит нет и счётчиков.
    (crate::dom::host_content(&style, me, vcx).is_some() && style.display != Some(Display::None))
        .then_some(style)
}
