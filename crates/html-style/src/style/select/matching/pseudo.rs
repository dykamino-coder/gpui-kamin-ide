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

/// Выполняется ли ОДИН псевдокласс на узле — для дополнительных
/// псевдоклассов компаунда (основной решает `matches`, слои — отбор
/// по имени). Неизвестный или слойный (`:hover`) здесь считается
/// НЕвыполненным: базовый каскад такое правило не применяет.
pub(super) fn pseudo_holds(
    sel: &Selector,
    pseudo: &str,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
) -> bool {
    if let Some(_inner) = pseudo
        .strip_prefix("not(")
        .and_then(|r| r.strip_suffix(')'))
    {
        return sel
            .prepared(pseudo)
            .filter(|p| p.single)
            .and_then(|p| p.list.first())
            .is_some_and(|inner| !matches(inner, me, path, sibs));
    }
    if let Some(ok) = is_holds(sel, pseudo, me, path, sibs) {
        return ok;
    }
    if pseudo == "empty" {
        return me.empty;
    }
    if pseudo == "link" || pseudo == "visited" {
        return me.href.is_some() && (pseudo == "visited") == me.visited;
    }
    if let Some(want) = pseudo
        .strip_prefix("dir(")
        .and_then(|r| r.strip_suffix(')'))
    {
        let rtl = me
            .dir
            .or_else(|| path.iter().rev().find_map(|a| a.dir))
            .unwrap_or(false);
        return want.trim().eq_ignore_ascii_case("rtl") == rtl;
    }
    if pseudo == "target-current" {
        return me.target_current;
    }
    if pseudo == "root" {
        return me.tag == "html";
    }
    if pseudo.starts_with("has-slotted") {
        return has_slotted_holds(sel, pseudo, me);
    }
    if let Some(want) = pseudo
        .strip_prefix("lang(")
        .and_then(|r| r.strip_suffix(')'))
    {
        return lang_matches(want, me, path);
    }
    if let Some(arg) = pseudo
        .strip_prefix("has(")
        .and_then(|r| r.strip_suffix(')'))
    {
        return me.has_marks.contains(&has_id(arg));
    }
    if let Some(ok) = nth_of_holds(sel, pseudo, me, path, sibs) {
        return ok;
    }
    structural(pseudo, me.spot).unwrap_or(false)
}

pub(super) fn is_host_pseudo(pseudo: &str) -> bool {
    pseudo == "host" || pseudo.starts_with("host(")
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

/// `:is()`/`:where()` (и устаревшие `:matches()`/`:any()`): узел совпадает
/// хоть с одним ГОДНЫМ селектором списка; негодные части прощаются
/// (selectors-4 §4.2 «forgiving selector list»). `None` — не этот псевдокласс.
pub(super) fn is_holds(
    sel: &Selector,
    pseudo: &str,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
) -> Option<bool> {
    let (name, arg) = pseudo.split_once('(')?;
    if !matches!(name, "is" | "where" | "matches" | "any") {
        return None;
    }
    let _arg = arg.strip_suffix(')')?;
    Some(
        sel.prepared(pseudo)
            .is_some_and(|p| p.list.iter().any(|inner| matches(inner, me, path, sibs))),
    )
}
