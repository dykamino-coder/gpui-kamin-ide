//! Сопоставление правил с элементом при ограничении `:visited` (Selectors 4
//! §8.1; Stylo `VisitedHandlingMode`, Blink `kMatchVisited`/`kMatchLink`):
//! все ссылки сопоставляются непосещёнными, кроме «своей» ссылки элемента
//! (он сам или ближайший предок-ссылка) во втором, посещённом, проходе.
//! Правило, совпавшее лишь в одном проходе, получает признак
//! [`LinkMatch`]: каскад берёт из него только свойства своего состояния.

use crate::style::css::{LinkMatch, Rule, Selector};
use crate::style::select::matching::matches;
use crate::style::select::{Ancestor, Sibs};

/// Совпавшие правила таблицы `rules`: номер и состояние ссылки.
pub(super) fn link_matches(
    rules: &[Rule],
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
) -> Vec<(usize, LinkMatch)> {
    let plain = |me: &Ancestor, path: &[Ancestor]| -> Vec<(usize, LinkMatch)> {
        rules
            .iter()
            .enumerate()
            .filter(|(_, r)| matches(&r.sel, me, path, sibs))
            .map(|(i, _)| (i, LinkMatch::Any))
            .collect()
    };
    if !me.visited && path.iter().all(|a| !a.visited) {
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
    rules
        .iter()
        .enumerate()
        .filter_map(|(i, r)| {
            let unvisited = matches(&r.sel, &me_u, &path_u, sibs);
            let visited = if mentions_link_state(&r.sel) {
                matches(&r.sel, &me_v, &path_v, sibs)
            } else {
                unvisited
            };
            LinkMatch::of(unvisited, visited).map(|link| (i, link))
        })
        .collect()
}

/// Упоминает ли селектор состояние ссылки (`:link`, `:visited`, в том
/// числе внутри `:is()`/`:not()`): иначе оба прохода совпадают.
fn mentions_link_state(sel: &Selector) -> bool {
    let named = |p: &str| {
        let p = p.to_ascii_lowercase();
        p.contains("link") || p.contains("visited")
    };
    sel.pseudo.as_deref().is_some_and(named)
        || sel.also.iter().any(|p| named(p))
        || sel
            .ancestor
            .as_ref()
            .is_some_and(|a| mentions_link_state(&a.0))
        || sel.prev.as_ref().is_some_and(|p| mentions_link_state(&p.0))
}
