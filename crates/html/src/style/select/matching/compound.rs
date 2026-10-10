//! Сопоставление составного селектора и цепочки комбинаторов (потомок, ребёнок, соседи).

use super::*;

/// Предок `path[at]` — предмет компаунда `sel` вместе с его соседним
/// комбинатором и цепочкой выше.
///
/// Сосед ПРЕДКА (`div + div span`, Selectors-4 §16.3/§16.4) проверяется по
/// братьям предка, сохранённым в его паспорте (`Ancestor::peers`): у соседа
/// те же предки — `path[..at]`. Прежде такое правило не совпадало никогда
/// (`ch-unit-001`: ширина `div + div span` терялась). Без сохранённых братьев
/// (обход вне `walk`) — честно не совпадает, как раньше.
pub(super) fn ancestor_holds(sel: &Selector, path: &[Ancestor], at: usize) -> bool {
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

pub(super) fn matches_compound(sel: &Selector, node: &Ancestor) -> bool {
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
