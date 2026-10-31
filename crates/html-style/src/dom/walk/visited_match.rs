//! Сопоставление правил с элементом при ограничении `:visited` (Selectors 4
//! §8.1; Stylo `VisitedHandlingMode`, Blink `kMatchVisited`/`kMatchLink`):
//! все ссылки сопоставляются непосещёнными, кроме «своей» ссылки элемента
//! (он сам или ближайший предок-ссылка) во втором, посещённом, проходе.
//! Правило, совпавшее лишь в одном проходе, получает признак
//! [`LinkMatch`]: каскад берёт из него только свойства своего состояния.

use crate::style::css::{LinkMatch, Rule};
use crate::style::select::matching::matches;
use crate::style::select::{Ancestor, Sibs};

/// Совпавшие правила таблицы `rules`: номер и состояние ссылки.
pub(super) fn link_matches(
    rules: &[Rule],
    index: &crate::style::select::rule_index::RuleIndex,
    candidates: &[usize],
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
) -> Vec<(usize, LinkMatch)> {
    let plain = |me: &Ancestor, path: &[Ancestor]| -> Vec<(usize, LinkMatch)> {
        candidates
            .iter()
            .copied()
            .filter(|&i| matches(&rules[i].sel, me, path, sibs))
            .map(|i| (i, LinkMatch::Any))
            .collect()
    };
    if !candidates.iter().any(|&i| index.link_state[i])
        || !me.visited && path.iter().all(|a| !a.visited)
    {
        return plain(me, path);
    }
    let unvisit = |a: &Ancestor| Ancestor {
        visited: false,
        ..a.clone()
    };
    let me_u = unvisit(me);
    let path_u: Vec<Ancestor> = path.iter().map(unvisit).collect();
    // Своя ссылка: сам элемент, иначе ближайший предок-ссылка.
    let (me_v, path_v) = if me.href.is_some() {
        (me.clone(), path_u.clone())
    } else {
        let mut path_v = path_u.clone();
        if let Some(i) = path.iter().rposition(|a| a.href.is_some()) {
            path_v[i] = path[i].clone();
        }
        (me_u.clone(), path_v)
    };
    let relevant_visited = me_v.visited || path_v.iter().any(|a| a.visited);
    if !relevant_visited {
        return plain(&me_u, &path_u);
    }
    candidates
        .iter()
        .copied()
        .filter_map(|i| {
            let r = &rules[i];
            let unvisited = matches(&r.sel, &me_u, &path_u, sibs);
            let visited = if index.link_state[i] {
                matches(&r.sel, &me_v, &path_v, sibs)
            } else {
                unvisited
            };
            LinkMatch::of(unvisited, visited).map(|link| (i, link))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::{BudgetKind, DocumentBudget, DocumentLimits, ResourceGovernor};
    use crate::dom::tree_index::TreeIndex;
    use crate::style::select::{census_of, rule_index::RuleIndex};

    #[test]
    fn rules_without_link_state_are_matched_once_even_below_visited_ancestors() {
        let dom = crate::dom::xml::xml_dom("<a href='x'><x/></a>").unwrap();
        let doc = crate::values::DocumentValues::detached(Default::default());
        let owner =
            DocumentBudget::new(DocumentLimits::default(), ResourceGovernor::default()).unwrap();
        let revision = owner.revision();
        doc.admit_revision(revision.clone(), None);
        let tree = TreeIndex::new(doc, true);
        let parent = dom.document.children.borrow()[0].clone();
        let (_, mut ancestors) = census_of(&dom.document.children.borrow(), &tree);
        let mut ancestor = ancestors.remove(0);
        ancestor.visited = true;
        let (_, mut children) = census_of(&parent.children.borrow(), &tree);
        let me = children.remove(0);
        let rules = [Rule {
            sel: crate::style::css::Selector::parse("x").unwrap(),
            decls: Default::default(),
            order: 0,
            origin: crate::style::css::Origin::Author,
            layer: vec![u32::MAX],
            link: LinkMatch::Any,
        }];
        let index = RuleIndex::new(&rules, &Default::default()).unwrap();
        let before = revision.used(BudgetKind::TraversalVisits);
        assert_eq!(
            link_matches(&rules, &index, &[0], &me, &[ancestor], Sibs::EMPTY),
            [(0, LinkMatch::Any)]
        );
        assert_eq!(revision.used(BudgetKind::TraversalVisits) - before, 1);
    }
}
