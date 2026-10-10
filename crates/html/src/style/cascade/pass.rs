//! Computed::apply_pass: один проход объявлений (обычные или !important) с подстановкой var()/attr(), all, порядком логических и физических свойств.

use super::*;
mod all_reset;

impl Computed {
    /// Один проход: только обычные объявления либо только важные.
    pub(crate) fn apply_pass<'a>(&mut self, d: &'a Decls, vars: &Decls, important: bool) {
        // Порядок ЗАПИСИ решает только между СОКРАЩЕНИЕМ и его длинным
        // свойством (`background` и `background-color`, `border` и
        // `border-width`): там он и виден — `background-color: red;
        // background: green` обязано дать зелёный, а обратная запись красный,
        // и словарь без порядка давал одно и то же.
        //
        // Между СОСЕДЯМИ (`border-width` и `border-style`, `white-space` и
        // `overflow-wrap`) порядок записи НЕ применяется: замерено, что от
        // него CSS2 теряет `border-width-012`, а CSS3 — девять пар
        // `textarea-pre-wrap-*`; наши свойства кое-где читают состояние друг
        // друга на применении, и полный порядок вскрывает эту зависимость.
        // Возвращать полный порядок вместе с независимым применением
        // объявлений.
        let order: Vec<&str> = d
            .get(crate::style::css::ORDER_KEY)
            .map(|s| s.split(crate::style::css::DECL_SEP).collect())
            .unwrap_or_default();
        // Внутри СЕМЬИ (сокращение и его длинные свойства) порядок — по
        // записи; сами семьи идут прежним порядком «сперва общее».
        // Семья — только НАСТОЯЩЕЕ сокращение со своими длинными свойствами.
        // По одному лишь общему началу судить нельзя: `overflow-wrap` не
        // часть `overflow`, и перестановка этой пары ЗАМЕРЕНА в минус —
        // девять пар `textarea-pre-wrap-*` уходят 0.00 → 0.76. Список
        // расширять по одному, каждое имя — со своим замером.
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО: добавить сюда `border` и `grid` (со своими
        // исключениями: `border-spacing`/`border-collapse`/`border-radius`/
        // `border-image` не части `border`, `grid-gap` не часть `grid`).
        // Статический просмотр нашёл 6 красных пар, где длинное свойство
        // `border-*` стоит ПЕРЕД сокращением, и 13 таких же с `grid`, но срез
        // из 1817 пар (border/grid/gap/margin-collapse/ch-units/line-names)
        // дал 1290 → 1290: ни одной пары в любую сторону. Значит порядок в
        // этих парах не решает — держат их другие корни.
        const SHORTHANDS: &[&str] = &["background"];
        let семья = |k: &'a str| -> &'a str {
            if font_members::contains(k) && d.contains_key("font") {
                return "font";
            }
            // A side shorthand (`border-right: 12px solid`) resets that
            // side's color to `currentColor`; a later `border-color: pink`
            // must win over it (css-cascade-4 §6.4: order of appearance).
            // Sorted by name the pair always ran `border-color` first, and
            // the side stayed black (css-gaps `grid-gap-decorations-*-ref`
            // `.col-rule`). Only side shorthands and the three all-side
            // shorthands share the order — `border-width`/`border-style`
            // among themselves keep the old order (see the note above).
            const EDGE: &[&str] = &[
                "border-top",
                "border-right",
                "border-bottom",
                "border-left",
                "border-color",
                "border-style",
                "border-width",
            ];
            if EDGE.contains(&k)
                && EDGE[..4].iter().any(|s| d.contains_key(*s))
                && EDGE[4..].iter().any(|s| d.contains_key(*s))
            {
                return "border-edge";
            }
            for root in SHORTHANDS {
                if k.len() > root.len()
                    && k.starts_with(root)
                    && k.as_bytes().get(root.len()) == Some(&b'-')
                    && d.contains_key(*root)
                {
                    return root;
                }
            }
            k
        };
        let место = |k: &str| order.iter().position(|n| *n == k).unwrap_or(usize::MAX);
        let mut keys: Vec<&'a String> = d.keys().collect();
        keys.sort_by_cached_key(|k| {
            let root = семья(k.as_str());
            (
                root.matches('-').count(),
                root,
                if order.is_empty() { 0 } else { место(k) },
                k.as_str(),
            )
        });
        let ordered = keys;
        // `all: revert` / `all: revert-layer` (css-cascade-5 §3.2 «all»,
        // §7.2): откат КАЖДОГО свойства, кроме `direction` и `unicode-bidi`,
        // тем же правилом, что откат одного свойства ниже, — всё, что этот
        // блок сказал о свойстве ДО `all`, снимается. Ключ `all` не
        // разбирался вовсе, и `background-color: red; border-color: red;
        // all: revert` оставлял красные поля ввода (`appearance-revert-001`).
        // Порядок — по первому появлению ключа (`ORDER_KEY`): повтор
        // свойства ПОСЛЕ `all` в том же блоке этим не различается (редкость).
        let all_at = d
            .get("all")
            .and_then(|v| {
                v.split(crate::style::css::DECL_SEP)
                    .rfind(|part| is_important(part) == important)
            })
            .filter(|part| {
                matches!(
                    strip_important(part).trim(),
                    "revert" | "revert-layer" | "initial" | "unset"
                )
            })
            .map(|part| (место("all"), strip_important(part).trim() == "initial"));
        // `all: initial` / `all: unset` (css-cascade-5 §3.2) сбрасывает ВСЁ,
        // что каскад сказал до него, — не только в этом блоке, но и в ранних
        // правилах и в таблице агента: `div` становится строчным, цвет и
        // шрифт — начальными (`initial`) или наследуемыми (`unset`). Кроме
        // `direction` и `unicode-bidi`. Прежде ключ `all` понимал только
        // откат, и `.test { all: initial }` оставлял красную рамку, фон и
        // флоат раннего правила (`all-prop-001/002`). Сброс — СВОЙ шаг
        // прохода: важные объявления ранних правил применяются позже и
        // переживают его, как велит §6.1.
        let all_reset = all_at.filter(|_| {
            d.get("all")
                .and_then(|v| {
                    v.split(crate::style::css::DECL_SEP)
                        .rfind(|part| is_important(part) == important)
                })
                .is_some_and(|part| matches!(strip_important(part).trim(), "initial" | "unset"))
        });
        let all_at = all_at.map(|(at, _)| at);
        if let Some((_, initial)) = all_reset {
            self.reset_for_all(initial);
        }
        for k in &ordered {
            let Some(v) = d.get(*k) else { continue };
            if k.starts_with("--")
                || k.as_str() == crate::style::css::ORDER_KEY
                || k.starts_with(crate::style::css::CUSTOM_IMPORTANT)
            {
                continue;
            }
            if let Some(at) = all_at
                && место(k.as_str()) < at
                && !matches!(k.as_str(), "direction" | "unicode-bidi")
            {
                continue;
            }
            // `revert`/`revert-layer` — не ЗНАЧЕНИЕ, а откат каскада
            // (css-cascade-5 §7.2, §7.3): объявление отменяет всё, что этот же
            // блок сказал о свойстве в ту же важность, — блок целиком лежит в
            // одном слое и одном происхождении, откатывать внутри него некуда.
            // Пока слово уезжало в разбор значения, оно там не читалось,
            // объявление выходило негодным (§4.1.7) — и прежнее `red` из того
            // же блока переживало откат (`revert-layer-001`: сплошной красный
            // квадрат вместо зелёного).
            let parts: Vec<&str> = v
                .split(crate::style::css::DECL_SEP)
                .filter(|part| is_important(part) == important)
                .collect();
            // САМО слово откатa по-прежнему уходит в `apply_one`: для 33
            // свойств из `initial_value()` он значит сброс к начальному, и
            // трогать это поведение здесь незачем.
            let from = parts
                .iter()
                .rposition(|part| matches!(strip_important(part).trim(), "revert" | "revert-layer"))
                .unwrap_or(0);
            for part in parts[from..].iter().copied() {
                // Типизированный `attr()` подставляется тем же шагом, что и
                // `var()` (css-values-5 §7.7): после него значение разбирается
                // как обычное.
                let resolved = resolve_sibling(resolve_attrs(
                    k.as_str(),
                    &resolve_vars(strip_important(part), vars),
                ));
                // CSS Variables §3: invalid after substitution means unset;
                // a preceding specified color cannot survive the computed value.
                if k.as_str() == "color"
                    && crate::style::css::variable_values::has_var(strip_important(part))
                    && Color::parse(&resolved).is_none()
                    && !matches!(
                        resolved.trim().to_ascii_lowercase().as_str(),
                        "inherit" | "initial" | "unset" | "revert" | "revert-layer"
                    )
                {
                    self.apply_one(k, "unset");
                } else {
                    self.apply_one(k, &resolved);
                }
            }
        }
    }
}
