//! Псевдоклассы при сопоставлении: :lang() с расширенной фильтрацией, общие псевдоклассы,
//! :host/:host(), ::slotted/:has-slotted.

use super::*;

/// `:lang(x)` — язык узла: свой атрибут `lang`, иначе ближайшего предка.
/// Совпадение — точное или по префиксу до дефиса, ASCII-регистронезависимо
/// (селекторы-4 §lang-pseudo; `fi` не совпадает с `fil`).
pub(super) fn lang_matches(want: &str, me: &Ancestor, path: &[Ancestor]) -> bool {
    let Some(lang) = language::effective(me, path) else {
        return false;
    };
    // Список диапазонов через запятую (`:lang(de, nl, fr)`), каждый —
    // идентификатор или строка; совпадение с любым.
    want.split(',').any(|range| {
        let range = range
            .trim()
            .trim_matches(|c| c == '"' || c == char::from(39));
        if range.is_empty() || range == "*" {
            return !lang.is_empty();
        }
        extended_lang_filter(range, lang)
    })
}

/// Расширенная фильтрация RFC 4647 §3.3.2 (Selectors-4 §7.2 «:lang()»):
/// первый подтег совпадает или диапазон `*`; дальше каждый подтег
/// диапазона ищется в теге по порядку, пропуская несовпавшие подтеги тега,
/// но не перескакивая через одиночный (`x`, `u`…). `*` в середине
/// диапазона пропускается. `*-FR` совпадает с `fr-Latn-FR`, `fr-FR` — тоже.
fn extended_lang_filter(range: &str, tag: &str) -> bool {
    let mut r = range.split('-');
    let mut t = tag.split('-');
    let (Some(r0), Some(t0)) = (r.next(), t.next()) else {
        return false;
    };
    if r0 != "*" && !r0.eq_ignore_ascii_case(t0) {
        return false;
    }
    let mut t_cur = t.next();
    for rs in r {
        if rs == "*" {
            continue;
        }
        loop {
            let Some(ts) = t_cur else { return false };
            if ts.eq_ignore_ascii_case(rs) {
                t_cur = t.next();
                break;
            }
            if ts.len() == 1 {
                return false;
            }
            t_cur = t.next();
        }
    }
    true
}

/// Выполняется ли ОДИН псевдокласс на узле: предметный (`matches`) и
/// дополнительные псевдоклассы компаунда решаются по виду, разобранному
/// вместе с селектором. Неизвестный или слойный (`:hover`) здесь считается
/// НЕвыполненным: базовый каскад такое правило не применяет, а `:host` вне
/// тени «matches nothing» (css-shadow-1 §3.1).
pub(super) fn pseudo_holds(
    sel: &Selector,
    pseudo: &str,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
) -> bool {
    let (prepared, kind) = sel.pseudo_of(pseudo);
    match kind {
        PseudoKind::Not => prepared
            .filter(|p| p.single)
            .and_then(|p| p.list.first())
            .is_some_and(|inner| !matches(inner, me, path, sibs)),
        PseudoKind::Is => list_holds(prepared, me, path, sibs),
        PseudoKind::Empty => me.empty,
        // `:visited` — адрес в истории (`link_visited`, с ограничением
        // `:visited` каскада элемента).
        PseudoKind::Link => me.href.is_some() && !me.visited,
        PseudoKind::Visited => me.href.is_some() && me.visited,
        // Своё направление, иначе ближайшего предка с `dir`; по умолчанию
        // слева направо (селекторы-4 §direction-pseudo).
        PseudoKind::Dir(rtl) => {
            rtl == me
                .dir
                .or_else(|| path.iter().rev().find_map(|a| a.dir))
                .unwrap_or(false)
        }
        PseudoKind::TargetCurrent => me.target_current,
        // Корень документа — `<html>`: не структурный по месту среди соседей.
        PseudoKind::Root => me.tag == "html",
        PseudoKind::HasSlotted => has_slotted_holds(sel, pseudo, me),
        PseudoKind::Lang => pseudo
            .strip_prefix("lang(")
            .and_then(|r| r.strip_suffix(')'))
            .is_some_and(|want| lang_matches(want, me, path)),
        PseudoKind::Has(id) => me.has_marks.contains(&id),
        PseudoKind::NthOf(backwards) => {
            nth_of_holds_prepared(sel, pseudo, prepared, backwards, me, path, sibs)
        }
        PseudoKind::Structural(position, of_type) => {
            structural_at(position, of_type, prepared, me.spot)
        }
        PseudoKind::Host | PseudoKind::UserAction | PseudoKind::Other => false,
    }
}

/// Хоть один годный селектор списка `:is()`/`:where()` совпадает
/// (selectors-4 §4.2 «forgiving selector list»).
fn list_holds(prepared: Option<&Prepared>, me: &Ancestor, path: &[Ancestor], sibs: Sibs) -> bool {
    prepared.is_some_and(|p| p.list.iter().any(|inner| matches(inner, me, path, sibs)))
}

/// `:host` / `:host(S)` на безликом хосте: голый совпадает всегда, с
/// аргументом — если хост В СВОЁМ СВЕТЛОМ КОНТЕКСТЕ совпадает с S
/// (css-shadow-1 §3.1 «in its normal context»; Blink `CheckPseudoHost`
/// сопоставляет в `element->GetTreeScope()`). Светлых братьев здесь нет:
/// `:first-child` в аргументе решается по `spot`, of-форма и `+`/`~` — нет.
pub(super) fn host_holds(sel: &Selector, pseudo: &str, node: &Ancestor) -> bool {
    if pseudo == "host" {
        return true;
    }
    let (Some(_arg), Some(light)) = (
        pseudo
            .strip_prefix("host(")
            .and_then(|r| r.strip_suffix(')')),
        &node.featureless,
    ) else {
        return false;
    };
    let Some(arg) = sel
        .prepared(pseudo)
        .filter(|p| p.single)
        .and_then(|p| p.list.first())
    else {
        return false;
    };
    let real = Ancestor {
        featureless: None,
        ..node.clone()
    };
    matches(arg, &real, &light[..], Sibs::EMPTY)
}

/// `:has-slotted` — у слота непуст список ПЛОСКИХ распределённых, включая
/// текст (`has-slotted-001` зелёная от одних пробелов); `:has-slotted(S)` —
/// среди них есть ЭЛЕМЕНТ, совпадающий с S в своём светлом контексте
/// (`functional-007`: `div + div` смотрит на светлых братьев). Не слот или
/// слот вне тени — не совпадает.
pub(super) fn has_slotted_holds(sel: &Selector, pseudo: &str, me: &Ancestor) -> bool {
    let Some(slot) = &me.slot else { return false };
    let Some(_arg) = pseudo
        .strip_prefix("has-slotted(")
        .and_then(|r| r.strip_suffix(')'))
    else {
        return !slot.flattened.is_empty();
    };
    let Some(prepared) = sel.prepared(pseudo) else {
        return false;
    };
    let list = &prepared.list;
    slot.flattened.iter().flatten().any(|c| {
        let sibs = Sibs {
            all: &c.all[..],
            pos: c.pos,
            is_elem: true,
            rc: None,
        };
        list.iter().any(|s| matches(s, &c.anc, &c.path[..], sibs))
    })
}

/// `:is()`/`:where()` (и устаревшие `:matches()`/`:any()`) по виду
/// псевдокласса. `None` — не этот псевдокласс.
pub(super) fn is_holds(
    sel: &Selector,
    pseudo: &str,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
) -> Option<bool> {
    match sel.pseudo_of(pseudo) {
        (prepared, PseudoKind::Is) => Some(list_holds(prepared, me, path, sibs)),
        _ => None,
    }
}
