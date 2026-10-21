//! Каскад: откат `revert`/`revert-layer` в объявлениях `style=""` до применения
//! значений. У встроенного стиля свой ранг в сравнении слоёв (CSS Cascade 5
//! §6.4): `revert-layer` снимает этот ранг, `revert` — ещё и авторские таблицы.
//! Возвращает копии правил и встроенных объявлений с удалёнными объявлениями.

use crate::style::css::{DeclBlock, Rule};

/// Inline styles have their own layer-comparison rank (CSS Cascade 5 §6.4).
/// `revert-layer` removes that rank; `revert` also removes author sheets.
pub(super) fn prepare(matched: &[&Rule], inline: &DeclBlock) -> Option<(Vec<Rule>, DeclBlock)> {
    let rollbacks: Vec<(&str, bool, bool)> = inline
        .names()
        .into_iter()
        // The all shorthand is handled by pass::all_reset.
        .filter(|key| *key != "all")
        .flat_map(|key| {
            [true, false].into_iter().filter_map(move |important| {
                let last = inline
                    .iter()
                    .rev()
                    .find(|d| d.name == key && d.important == important)?;
                let keyword = last.value.trim();
                matches!(keyword, "revert" | "revert-layer").then_some((
                    key,
                    important,
                    keyword == "revert",
                ))
            })
        })
        .collect();
    if rollbacks.is_empty() {
        return None;
    }
    let mut rules: Vec<Rule> = matched.iter().map(|r| (*r).clone()).collect();
    let mut kept = inline.clone();
    for (key, important, origin) in rollbacks {
        remove(&mut kept, key, important);
        if origin {
            for rule in rules.iter_mut().filter(|r| r.origin == 1) {
                remove(&mut rule.decls, key, important);
            }
        }
    }
    Some((rules, kept))
}

/// Снять объявления `key`, откатанные объявлением важности `important`:
/// обычный откат снимает обычные, важный — все.
fn remove(decls: &mut DeclBlock, key: &str, important: bool) {
    decls.retain(|d| d.name != key || (!important && d.important));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::values::value::Len;
    use crate::style::{
        computed::Computed,
        css::{Selector, parse_decls},
    };

    #[test]
    fn inline_rollback_preserves_origin_and_element_attached_rank() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let ua = Rule {
            sel: Selector::parse("p").unwrap(),
            decls: parse_decls("width: 10px"),
            origin: 0,
            order: 0,
            layer: vec![u32::MAX],
        };
        let author = Rule {
            decls: parse_decls("width: 20px"),
            origin: 1,
            ..ua.clone()
        };
        for (inline, expected) in [
            ("width: revert", 10.0),
            ("width: revert-layer", 20.0),
            ("width: 30px; width: revert !important", 10.0),
        ] {
            let c = Computed::resolve(&mut vec![&ua, &author], &parse_decls(inline), vcx);
            assert_eq!(c.width, Some(Len::Px(expected)), "{inline}");
        }
        let author = Rule {
            decls: parse_decls("width: revert-layer"),
            ..author
        };
        let c = Computed::resolve(&mut vec![&ua, &author], &DeclBlock::new(), vcx);
        assert_eq!(c.width, Some(Len::Px(10.0)));
    }
}
