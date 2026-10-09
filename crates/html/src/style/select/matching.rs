//! `matches` и структурные псевдоклассы.
// owner: B

use crate::dom::language;
use crate::style::css::Selector;
use crate::style::select::has::has_id;
use crate::style::select::{Ancestor, Sibs, Spot};

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

/// Псевдокласс — of-форма `:nth-child(… of S)`?
fn nth_of_form(pseudo: &str) -> bool {
    let Some((name, arg)) = pseudo.split_once('(') else {
        return false;
    };
    matches!(name, "nth-child" | "nth-last-child")
        && arg
            .strip_suffix(')')
            .is_some_and(|a| crate::style::css::nth_of_parts(a).is_some())
}

/// `:nth-child(An+B of S)` / `:nth-last-child(An+B of S)` (селекторы-4):
/// узел обязан сам совпасть с S, а номер считается только среди совпавших
/// братьев — с начала либо с конца. `None` — псевдокласс не of-формы.
fn nth_of_holds(pseudo: &str, me: &Ancestor, path: &[Ancestor], sibs: Sibs) -> Option<bool> {
    let (name, arg) = pseudo.split_once('(')?;
    let backwards = match name {
        "nth-child" => false,
        "nth-last-child" => true,
        _ => return None,
    };
    let arg = arg.strip_suffix(')')?;
    let (anb, list) = crate::style::css::nth_of_parts(arg)?;
    let hit = |a: &Ancestor, s: Sibs| list.iter().any(|sel| matches(sel, a, path, s));
    if !sibs.is_elem || list.is_empty() || !hit(me, sibs) {
        return Some(false);
    }
    let (peers, base) = if backwards {
        (sibs.next(), sibs.pos + 1)
    } else {
        (sibs.prev(), 0)
    };
    let idx = 1 + peers
        .iter()
        .enumerate()
        .filter(|(i, a)| hit(a, sibs.at(base + i)))
        .count();
    Some(nth_matches(&anb, idx))
}

/// `:lang(x)` — язык узла: свой атрибут `lang`, иначе ближайшего предка.
/// Совпадение — точное или по префиксу до дефиса, ASCII-регистронезависимо
/// (селекторы-4 §lang-pseudo; `fi` не совпадает с `fil`).
fn lang_matches(want: &str, me: &Ancestor, path: &[Ancestor]) -> bool {
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
fn pseudo_holds(pseudo: &str, me: &Ancestor, path: &[Ancestor], sibs: Sibs) -> bool {
    if let Some(inner) = pseudo
        .strip_prefix("not(")
        .and_then(|r| r.strip_suffix(')'))
    {
        return Selector::parse(inner).is_some_and(|inner| !matches(&inner, me, path, sibs));
    }
    if pseudo == "link" || pseudo == "visited" {
        let Some(href) = &me.href else { return false };
        let visited = href.is_empty() || href.starts_with('#');
        return (pseudo == "visited") == visited;
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
    if pseudo == "root" {
        return me.tag == "html";
    }
    if pseudo.starts_with("has-slotted") {
        return has_slotted_holds(pseudo, me);
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
    if let Some(ok) = nth_of_holds(pseudo, me, path, sibs) {
        return ok;
    }
    structural(pseudo, me.spot).unwrap_or(false)
}

fn is_host_pseudo(pseudo: &str) -> bool {
    pseudo == "host" || pseudo.starts_with("host(")
}

/// `:host` / `:host(S)` на безликом хосте: голый совпадает всегда, с
/// аргументом — если хост В СВОЁМ СВЕТЛОМ КОНТЕКСТЕ совпадает с S
/// (css-shadow-1 §3.1 «in its normal context»; Blink `CheckPseudoHost`
/// сопоставляет в `element->GetTreeScope()`). Светлых братьев здесь нет:
/// `:first-child` в аргументе решается по `spot`, of-форма и `+`/`~` — нет.
fn host_holds(pseudo: &str, node: &Ancestor) -> bool {
    if pseudo == "host" {
        return true;
    }
    let (Some(arg), Some(light)) = (
        pseudo.strip_prefix("host(").and_then(|r| r.strip_suffix(')')),
        &node.featureless,
    ) else {
        return false;
    };
    let Some(arg) = Selector::parse(arg) else {
        return false;
    };
    let real = Ancestor {
        featureless: None,
        ..node.clone()
    };
    matches(&arg, &real, &light[..], Sibs::EMPTY)
}

/// `:has-slotted` — у слота непуст список ПЛОСКИХ распределённых, включая
/// текст (`has-slotted-001` зелёная от одних пробелов); `:has-slotted(S)` —
/// среди них есть ЭЛЕМЕНТ, совпадающий с S в своём светлом контексте
/// (`functional-007`: `div + div` смотрит на светлых братьев). Не слот или
/// слот вне тени — не совпадает.
fn has_slotted_holds(pseudo: &str, me: &Ancestor) -> bool {
    let Some(slot) = &me.slot else { return false };
    let Some(arg) = pseudo
        .strip_prefix("has-slotted(")
        .and_then(|r| r.strip_suffix(')'))
    else {
        return !slot.flattened.is_empty();
    };
    let list: Vec<Selector> = crate::style::css::split_selector_list(arg)
        .into_iter()
        .filter_map(Selector::parse)
        .collect();
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

/// Структурные псевдоклассы: место узла среди соседей.
///
/// `None` — псевдокласс не структурный, решение принимает вызывающий.
fn structural(pseudo: &str, spot: Spot) -> Option<bool> {
    let (name, arg) = match pseudo.split_once('(') {
        Some((n, rest)) => (n, rest.trim_end_matches(')').trim()),
        None => (pseudo, ""),
    };
    let (index, total) = match name {
        "first-child" | "last-child" | "only-child" | "nth-child" | "nth-last-child" => {
            (spot.index, spot.total)
        }
        "first-of-type" | "last-of-type" | "only-of-type" | "nth-of-type" | "nth-last-of-type" => {
            (spot.of_type, spot.of_type_total)
        }
        _ => return None,
    };
    // Узел без места — не элемент; таким структурные правила не адресуются.
    if index == 0 {
        return Some(false);
    }
    Some(match name {
        "first-child" | "first-of-type" => index == 1,
        "last-child" | "last-of-type" => index == total,
        "only-child" | "only-of-type" => total == 1,
        "nth-child" | "nth-of-type" => nth_matches(arg, index),
        "nth-last-child" | "nth-last-of-type" => nth_matches(arg, total + 1 - index),
        _ => false,
    })
}

/// Запись `an+b` из `:nth-child()`: подходит ли номер.
fn nth_matches(arg: &str, index: usize) -> bool {
    let arg = arg.trim().to_ascii_lowercase();
    let (a, b) = match arg.as_str() {
        "odd" => (2i64, 1i64),
        "even" => (2, 0),
        _ => match arg.split_once('n') {
            None => match arg.parse::<i64>() {
                Ok(b) => (0, b),
                Err(_) => return false,
            },
            Some((head, tail)) => {
                // Пробел между множителем и `n` запрещён (css-syntax
                // §the-anb-type): `1 n` — не An+B.
                if head.ends_with(char::is_whitespace) {
                    return false;
                }
                let a = match head {
                    "" | "+" => 1,
                    "-" => -1,
                    other => match other.parse::<i64>() {
                        Ok(v) => v,
                        Err(_) => return false,
                    },
                };
                // Сдвиг после `n` обязан нести явный знак: `2n 1` — не An+B,
                // пробелы допустимы только вокруг самого знака.
                let tail = tail.trim();
                let b = if tail.is_empty() {
                    0
                } else {
                    let (sign, num) = match tail.strip_prefix('+') {
                        Some(rest) => (1i64, rest),
                        None => match tail.strip_prefix('-') {
                            Some(rest) => (-1, rest),
                            None => return false,
                        },
                    };
                    let num = num.trim_start();
                    // Второй знак у сдвига (`2n--1`) — не число.
                    if !num.bytes().all(|c| c.is_ascii_digit()) {
                        return false;
                    }
                    match num.parse::<i64>() {
                        Ok(v) => sign * v,
                        Err(_) => return false,
                    }
                };
                (a, b)
            }
        },
    };
    let index = index as i64;
    if a == 0 {
        return index == b;
    }
    let diff = index - b;
    diff % a == 0 && diff / a >= 0
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
    (0..path.len()).rev().any(|i| ancestor_holds(parent_sel, path, i))
}

/// Предок `path[at]` — предмет компаунда `sel` вместе с его соседним
/// комбинатором и цепочкой выше.
///
/// Сосед ПРЕДКА (`div + div span`, Selectors-4 §16.3/§16.4) проверяется по
/// братьям предка, сохранённым в его паспорте (`Ancestor::peers`): у соседа
/// те же предки — `path[..at]`. Прежде такое правило не совпадало никогда
/// (`ch-unit-001`: ширина `div + div span` терялась). Без сохранённых братьев
/// (обход вне `walk`) — честно не совпадает, как раньше.
fn ancestor_holds(sel: &Selector, path: &[Ancestor], at: usize) -> bool {
    if !matches_ancestor_compound(sel, &path[at]) {
        return false;
    }
    if sel.prev.is_some() {
        let Some((all, pos)) = &path[at].peers else {
            return false;
        };
        // Предок сопоставляется ПОЛНОСТЬЮ, как предмет: псевдоклассы его
        // компаунда (`* ~ :root div` — у корня братьев нет), соседи и цепочка
        // выше; братья предка делят с ним предков `path[..at]`.
        let sibs = Sibs {
            all: &all[..],
            pos: *pos,
            is_elem: true,
            rc: Some(all),
        };
        return matches(sel, &path[at], &path[..at], sibs);
    }
    matches_chain(sel, path, at)
}

/// Продолжение цепочки вверх для `.a .b .c`.
fn matches_chain(sel: &Selector, path: &[Ancestor], at: usize) -> bool {
    let Some(anc) = &sel.ancestor else {
        return true;
    };
    let (parent_sel, direct) = (&anc.0, anc.1);
    if direct {
        return at > 0 && ancestor_holds(parent_sel, path, at - 1);
    }
    (0..at).rev().any(|i| ancestor_holds(parent_sel, path, i))
}

/// Компаунд ПРЕДКА: как `matches_compound`, но псевдоклассы действия
/// пользователя на нём не выполняются.
fn matches_ancestor_compound(sel: &Selector, node: &Ancestor) -> bool {
    // Псевдоклассы действия пользователя у НЕ-предметного компаунда
    // (`grid:hover item[style]`): в неподвижном кадре ни наведения, ни
    // нажатия нет (selectors-4 §user-action: «matches while the user
    // designates an element»), а пропуск делал предка всегда наведённым —
    // правило красило всех потомков. Слой наведения строится только для
    // предметного `:hover` (`dom.rs`, `pseudo == "hover"`).
    let user_action = |p: &str| matches!(p, "hover" | "active");
    if sel.pseudo.as_deref().is_some_and(user_action) || sel.also.iter().any(|p| user_action(p)) {
        return false;
    }
    matches_compound(sel, node)
}

fn matches_compound(sel: &Selector, node: &Ancestor) -> bool {
    // Безликий хост: ни тег, ни `*`, ни класс, ни атрибут его не берут —
    // только `:host`/`:host(S)`, и все псевдоклассы компаунда обязаны быть
    // такими (`:host:host` — да, `div:host`, `:host.host` — нет;
    // `selectors/featureless-002`).
    if node.featureless.is_some() {
        let bare = sel.tag.is_none()
            && !sel.universal
            && sel.id.is_none()
            && sel.classes.is_empty()
            && sel.attrs.is_empty();
        return bare
            && sel.pseudo.as_deref().is_some_and(|p| host_holds(p, node))
            && sel.also.iter().all(|p| host_holds(p, node));
    }
    if let Some(t) = &sel.tag
        && t != &node.tag
    {
        return false;
    }
    if let Some(id) = &sel.id
        && node.id.as_deref() != Some(id.as_str())
    {
        return false;
    }
    // `:has()` НЕ-предметного компаунда (`div:has(.x) p`): отметка лежит
    // на самом узле - раньше псевдокласс здесь пропускался, и правило
    // красило все `div p` подряд.
    if let Some(p) = &sel.pseudo
        && let Some(arg) = p.strip_prefix("has(").and_then(|r| r.strip_suffix(')'))
        && !node.has_marks.contains(&has_id(arg))
    {
        return false;
    }
    // Структурный псевдокласс НЕ-предметного компаунда
    // (`td:nth-child(2) div`): место предка среди братьев известно из
    // `spot` — без проверки любой `td` подходил под любой номер, и
    // последнее правило перекрашивало все колонки
    // (logical-physical-mapping-001). Нестуктурные (`:hover`) здесь
    // по-прежнему пропускаются, of-форма — тоже: её решает предметный
    // путь по списку братьев, а по одному `spot` она не считается.
    if let Some(p) = &sel.pseudo
        && !nth_of_form(p)
        && let Some(ok) = structural(p, node.spot)
        && !ok
    {
        return false;
    }
    if !sel.attrs.iter().all(|a| {
        a.matches(
            node.attrs
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(&a.name))
                .map(|(_, v)| v.as_str()),
        )
    }) {
        return false;
    }
    sel.classes.iter().all(|c| node.classes.contains(c))
}
