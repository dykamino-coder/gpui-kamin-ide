//! Псевдоклассы и псевдоэлементы: известные имена, специфичность :is/:not/:where/:has, части :nth-*(… of S), деление списка селекторов.

use super::*;

/// Разбор внутренности атрибутного условия: `name`, `name=value`,
/// `name~="v" i` и родня. Кавычки значения снимаются, ` i` в хвосте —
/// регистронезависимость.
/// Известно ли имя псевдокласса или псевдоэлемента.
///
/// Перечень закрытый: по Selectors §3.1 неизвестное имя роняет весь список
/// селекторов, поэтому сюда входит и то, что мы разбираем, но не исполняем —
/// иначе правило с ним пропало бы целиком.
pub(super) fn known_pseudo(name: &str) -> bool {
    let head = name.split_once('(').map_or(name, |(h, _)| h);
    is_pseudo_element(head)
        || matches!(
            head,
            "hover"
                | "active"
                | "focus"
                | "focus-visible"
                | "focus-within"
                | "link"
                | "visited"
                | "any-link"
                | "target"
                | "target-within"
                | "root"
                | "empty"
                | "scope"
                | "checked"
                | "indeterminate"
                | "default"
                | "disabled"
                | "enabled"
                | "read-only"
                | "read-write"
                | "required"
                | "optional"
                | "valid"
                | "invalid"
                | "in-range"
                | "out-of-range"
                | "placeholder-shown"
                | "autofill"
                | "open"
                | "modal"
                | "fullscreen"
                | "picture-in-picture"
                | "defined"
                | "host"
                | "host-context"
                | "has-slotted"
                | "first-child"
                | "last-child"
                | "only-child"
                | "first-of-type"
                | "last-of-type"
                | "only-of-type"
                | "nth-child"
                | "nth-last-child"
                | "nth-of-type"
                | "nth-last-of-type"
                | "nth-col"
                | "nth-last-col"
                | "not"
                | "is"
                | "where"
                | "has"
                | "matches"
                | "any"
                | "lang"
                | "dir"
        )
}

/// ПсевдоЭЛЕМЕНТ (а не псевдокласс): после него составная часть кончается.
/// Имя может нести аргумент (`scroll-button(block-end)`) — сравнивается голова.
pub(super) fn is_pseudo_element(name: &str) -> bool {
    let head = name.split_once('(').map_or(name, |(h, _)| h);
    matches!(
        head,
        "before"
            | "after"
            | "first-line"
            | "first-letter"
            | "marker"
            | "placeholder"
            | "selection"
            | "backdrop"
            | "file-selector-button"
            // css-overflow-5: коробки собирает `dom.rs::scroll_marker_pass`.
            | "scroll-marker"
            | "scroll-marker-group"
            | "scroll-button"
    )
}

/// Список селекторов через запятую вне скобок - для `of S` и `:has()`.
pub(crate) fn split_selector_list(raw: &str) -> Vec<&str> {
    split_top_level(raw, ',')
}

/// Специфичность одного псевдокласса: +1 к классам, а `of S` у
/// `:nth-child`/`:nth-last-child` добавляет покомпонентный вес самого
/// специфичного селектора списка (селекторы-4 §specificity). `:has()` -
/// max по списку аргументов БЕЗ собственного веса псевдокласса.
pub(super) fn pseudo_specificity(pseudo: &str) -> (u32, u32, u32) {
    if let Some(arg) = pseudo
        .strip_prefix("has(")
        .and_then(|r| r.strip_suffix(')'))
    {
        return split_top_level(arg, ',')
            .into_iter()
            .filter_map(|one| {
                let one = one.trim();
                let rest = one.strip_prefix(['>', '+', '~']).unwrap_or(one);
                Selector::parse(rest).map(|s| s.specificity())
            })
            .max()
            .unwrap_or_default();
    }
    let mut s = (0u32, 1u32, 0u32);
    if let Some((name, arg)) = pseudo.split_once('(')
        && matches!(name, "nth-child" | "nth-last-child")
        && let Some(arg) = arg.strip_suffix(')')
        && let Some((_, list)) = nth_of_parts(arg)
    {
        let m = list
            .iter()
            .map(Selector::specificity)
            .max()
            .unwrap_or_default();
        s = (s.0 + m.0, s.1 + m.1, s.2 + m.2);
    }
    s
}

/// Разбор аргумента `:nth-child(An+B of S)`: An+B-часть и список S.
///
/// `of` ищется вне скобок, регистронезависимо, с границей идентификатора с
/// обеих сторон (перед ним пробел, после — не буква-цифра-дефис: `of.foo`
/// годится). Нет `of` — не of-форма; битая или пустая часть S делает весь
/// псевдокласс несопоставимым (вернётся пустой список — звать не с чем).
pub(crate) fn nth_of_parts(arg: &str) -> Option<(String, Vec<Selector>)> {
    let bytes = arg.as_bytes();
    let mut depth = 0i32;
    let mut i = 0usize;
    let mut split = None;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                i += 2;
                continue;
            }
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth -= 1,
            c if depth == 0
                && c.eq_ignore_ascii_case(&b'o')
                && bytes
                    .get(i + 1)
                    .is_some_and(|f| f.eq_ignore_ascii_case(&b'f'))
                && i > 0
                && bytes[i - 1].is_ascii_whitespace()
                && bytes
                    .get(i + 2)
                    .is_none_or(|c| !c.is_ascii_alphanumeric() && *c != b'-') =>
            {
                split = Some(i);
                break;
            }
            _ => {}
        }
        i += 1;
    }
    let at = split?;
    let anb = arg[..at].trim().to_string();
    let mut list = vec![];
    for one in split_top_level(&arg[at + 2..], ',') {
        let Some(sel) = Selector::parse(one) else {
            return Some((anb, vec![]));
        };
        list.push(sel);
    }
    Some((anb, list))
}
