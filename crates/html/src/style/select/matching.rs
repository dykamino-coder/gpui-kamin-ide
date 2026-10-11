//! `matches` и структурные псевдоклассы.
// owner: B

use crate::dom::language;
use crate::style::css::Selector;
use crate::style::select::has::has_id;
use crate::style::select::{Ancestor, Sibs, Spot};

mod compound;
mod nth;
mod pseudo;
use compound::{ancestor_holds, matches_compound};
use nth::{nth_of_form, nth_of_holds, structural};
use pseudo::{has_slotted_holds, host_holds, is_host_pseudo, lang_matches, pseudo_holds};

/// Сопоставление селектора с узлом и его цепочкой предков.
///
/// `sibs` — предыдущие соседи-элементы узла в порядке разметки: по ним
/// решаются соседние комбинаторы `+` и `~`.
pub(crate) fn matches(sel: &Selector, me: &Ancestor, path: &[Ancestor], sibs: Sibs) -> bool {
    // Безликий хост (изнутри своей тени): предков и братьев у него в этой
    // области нет, совпадает лишь `:host`-компаунд (см. `matches_compound`).
    if me.featureless.is_some() {
        return sel.ancestor.is_none() && sel.prev.is_none() && matches_compound(sel, me);
    }
    if let Some(pseudo) = &sel.pseudo {
        // `:host` вне тени «matches nothing» (css-shadow-1 §3.1).
        if is_host_pseudo(pseudo) {
            return false;
        }
        if pseudo.starts_with("has-slotted") {
            if !has_slotted_holds(pseudo, me) {
                return false;
            }
            return matches_ignoring_pseudo(sel, me, path, sibs);
        }
        // `:not(...)` — отрицание вложенного селектора. Разбирается здесь, а
        // не среди структурных: внутри скобок может стоять тег или класс, а им
        // нужен сам узел, а не только его место среди соседей.
        if let Some(inner) = pseudo
            .strip_prefix("not(")
            .and_then(|rest| rest.strip_suffix(')'))
        {
            let Some(inner) = Selector::parse(inner) else {
                return false;
            };
            if matches(&inner, me, path, sibs) {
                return false;
            }
            return matches_ignoring_pseudo(sel, me, path, sibs);
        }
        // Структурный псевдокласс — часть обычного каскада: он зависит только
        // от места узла в дереве. Остальные (`:hover`, `::before`) сюда не
        // попадают: их применяет отдельный слой при отрисовке.
        // `:root` — корень документа, то есть `<html>`. Он не структурный по
        // месту среди соседей, поэтому решается здесь: без него объявления
        // вроде `:root { font: 25px/1 Ahem }` не доезжали НИКУДА, и страница
        // набиралась шрифтом по умолчанию (`text-align-last-015`).
        // Ссылки: `:visited` — адрес уже в истории. Свою страницу браузер в
        // историю кладёт по определению, поэтому пустой `href` и якорь на
        // себя — посещённые; остальное для нас непосещённое.
        if pseudo == "link" || pseudo == "visited" {
            let Some(href) = &me.href else { return false };
            let visited = href.is_empty() || href.starts_with('#');
            if (pseudo == "visited") != visited {
                return false;
            }
            return matches_ignoring_pseudo(sel, me, path, sibs);
        }
        // `:dir(rtl|ltr)` — направление узла: свой атрибут `dir`, иначе
        // ближайшего предка с ним; по умолчанию письмо слева направо
        // (селекторы-4 §direction-pseudo).
        if let Some(want) = pseudo
            .strip_prefix("dir(")
            .and_then(|r| r.strip_suffix(')'))
        {
            let rtl = me
                .dir
                .or_else(|| path.iter().rev().find_map(|a| a.dir))
                .unwrap_or(false);
            if want.trim().eq_ignore_ascii_case("rtl") != rtl {
                return false;
            }
            return matches_ignoring_pseudo(sel, me, path, sibs);
        }
        if pseudo == "root" {
            if me.tag != "html" {
                return false;
            }
            return matches_ignoring_pseudo(sel, me, path, sibs);
        }
        if let Some(want) = pseudo
            .strip_prefix("lang(")
            .and_then(|r| r.strip_suffix(')'))
        {
            if !lang_matches(want, me, path) {
                return false;
            }
            return matches_ignoring_pseudo(sel, me, path, sibs);
        }
        if let Some(arg) = pseudo
            .strip_prefix("has(")
            .and_then(|r| r.strip_suffix(')'))
        {
            if !me.has_marks.contains(&has_id(arg)) {
                return false;
            }
            return matches_ignoring_pseudo(sel, me, path, sibs);
        }
        if let Some(ok) = nth_of_holds(pseudo, me, path, sibs) {
            if !ok {
                return false;
            }
            return matches_ignoring_pseudo(sel, me, path, sibs);
        }
        let Some(ok) = structural(pseudo, me.spot) else {
            return false;
        };
        if !ok {
            return false;
        }
    }
    matches_ignoring_pseudo(sel, me, path, sibs)
}

/// То же сопоставление, но без отсева по псевдоклассу — для слоя наведения.
pub(crate) fn matches_ignoring_pseudo(
    sel: &Selector,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
) -> bool {
    if !matches_compound(sel, me) {
        return false;
    }
    // Дополнительные псевдоклассы компаунда (`li:first-child:last-child`)
    // обязаны выполниться ВСЕ; раньше выживал только последний.
    if !sel.also.iter().all(|p| pseudo_holds(p, me, path, sibs)) {
        return false;
    }
    // Соседний комбинатор: `+` — ровно предыдущий сосед-элемент, `~` — любой
    // раньше. Сосед проверяется ПОЛНЫМ сопоставлением со своими соседями
    // слева и тем же путём предков (соседи его делят).
    if let Some(prev) = &sel.prev {
        let (prev_sel, adjacent) = (&prev.0, prev.1);
        let prev = sibs.prev();
        let hit = |i: usize| matches(prev_sel, &prev[i], path, sibs.at(i));
        let found = if adjacent {
            !prev.is_empty() && hit(prev.len() - 1)
        } else {
            (0..prev.len()).rev().any(hit)
        };
        if !found {
            return false;
        }
    }
    let Some(anc) = &sel.ancestor else {
        return true;
    };
    let (parent_sel, direct) = (&anc.0, anc.1);
    if direct {
        return !path.is_empty() && ancestor_holds(parent_sel, path, path.len() - 1);
    }
    (0..path.len())
        .rev()
        .any(|i| ancestor_holds(parent_sel, path, i))
}
