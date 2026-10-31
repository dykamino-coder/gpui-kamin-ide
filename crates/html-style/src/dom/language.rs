//! Язык содержимого по `lang`/`xml:lang` — общий для селекторов и автоматических
//! кавычек.
//!
//! `effective` — язык узла или ближайшего предка; `inherited` — то же как
//! наследуемое значение обхода (O(1) на узел); `parent` — язык родителя
//! для кавычек элемента (css-content-3 §quotes-property: у корня — свой);
//! `pseudo` — явное `quotes: auto` на псевдоэлементе выбирает язык
//! порождающего элемента и ставит его счётчикам кавычек.

use crate::style::select::Ancestor;

fn own(node: &Ancestor) -> Option<&str> {
    own_attr(&node.attrs)
}

fn own_attr(attrs: &[(String, String)]) -> Option<&str> {
    attrs
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("lang") || k.eq_ignore_ascii_case("xml:lang"))
        .map(|(_, value)| value.as_str())
}

pub fn effective<'a>(node: &'a Ancestor, path: &'a [Ancestor]) -> Option<&'a str> {
    own(node).or_else(|| path.iter().rev().find_map(own))
}

/// Язык узла для обхода коробок — наследуемое значение: свой атрибут, иначе
/// переданный вниз язык родителя; поиск по цепочке — только у паспорта,
/// собранного вне обхода (граница тени или слота).
pub(super) fn inherited(attrs: &[(String, String)], path: &[Ancestor]) -> std::rc::Rc<str> {
    match own_attr(attrs) {
        Some(lang) => lang.into(),
        None => match path.split_last() {
            Some((parent, ancestors)) => of(parent, ancestors),
            None => "".into(),
        },
    }
}

/// Язык узла: переданный обходом, иначе по цепочке предков.
fn of(node: &Ancestor, path: &[Ancestor]) -> std::rc::Rc<str> {
    node.lang
        .clone()
        .unwrap_or_else(|| effective(node, path).unwrap_or("").into())
}

/// CSS Content 3 §quotes-property: root auto uses its own language.
pub(super) fn parent(node: &Ancestor, path: &[Ancestor]) -> std::rc::Rc<str> {
    if let Some((parent, ancestors)) = path.split_last() {
        of(parent, ancestors)
    } else {
        own(node).unwrap_or("").into()
    }
}

/// Explicit auto on a pseudo selects its originating element's language.
pub(super) fn pseudo(
    counters: &mut crate::style::generated::counters::Counters,
    style: &crate::style::computed::Computed,
    node: &Ancestor,
    path: &[Ancestor],
) {
    if style
        .quotes
        .as_ref()
        .and_then(Option::as_ref)
        .is_some_and(Vec::is_empty)
    {
        counters.set_quote_language(&of(node, path));
    }
}
