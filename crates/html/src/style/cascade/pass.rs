//! Computed::apply_pass: один проход объявлений блока (обычные или !important) с подстановкой
//! var()/attr(), all, порядком сокращений и их длинных свойств.

use super::*;
use crate::document::state::ValueCx;
use crate::style::css::{DeclBlock, Declaration, VarMap};
mod all_reset;

impl Computed {
    /// Один блок объявлений без словаря переменных — тем же путём, что и
    /// каскад правил ([`Self::apply_decls_with_vars`]): обычные, затем
    /// `!important`, порядок свойств детерминирован (`apply_pass`).
    pub fn apply_decls(&mut self, d: &DeclBlock, vcx: ValueCx<'_>) {
        self.apply_decls_with_vars(d, &VarMap::default(), vcx);
    }

    /// То же, но со словарём переменных: `var(--x)` подставляется значением.
    ///
    /// Без этого современные темы не работают вовсе — они целиком построены на
    /// переменных, и каждое такое объявление молча терялось.
    pub fn apply_decls_with_vars(&mut self, d: &DeclBlock, vars: &VarMap, vcx: ValueCx<'_>) {
        // Два прохода: сначала обычные объявления, затем помеченные
        // `!important`. Так важное перекрывает любое обычное независимо от
        // порядка правил.
        for important in [false, true] {
            self.apply_pass(d, vars, important, CascadeCx::NONE, vcx);
        }
    }

    /// Один проход: только обычные объявления либо только важные.
    pub(crate) fn apply_pass(
        &mut self,
        d: &DeclBlock,
        vars: &VarMap,
        important: bool,
        cx: CascadeCx,
        vcx: ValueCx<'_>,
    ) {
        // Свойства блока по первому появлению; пользовательские свойства
        // разрешены своим каскадом (`custom_properties`) и здесь не
        // применяются.
        let names: Vec<&str> = d
            .names()
            .into_iter()
            .filter(|k| !k.starts_with("--"))
            .collect();
        // Порядок ЗАПИСИ решает только между СОКРАЩЕНИЕМ и его длинным
        // свойством (`background` и `background-color`, `border` и
        // `border-width`): там он и виден — `background-color: red;
        // background: green` обязано дать зелёный, а обратная запись красный.
        //
        // Между СОСЕДЯМИ (`border-width` и `border-style`, `white-space` и
        // `overflow-wrap`) порядок записи НЕ применяется: от него ломаются
        // `border-width-012` и `textarea-pre-wrap-*` — наши свойства
        // кое-где читают состояние друг друга на применении, и полный
        // порядок вскрывает эту зависимость. Семьи идут «сперва общее»:
        // общность меряется числом дефисов (`border` < `border-width` <
        // `border-top-width`), и `border: solid gray; border-width: 1px 2px
        // 3px 4px` даёт 1/2/3/4 (`abs-pos-border-offset-001`). Возвращать
        // полный порядок вместе с независимым применением объявлений.
        //
        // Внутри СЕМЬИ (сокращение и его длинные свойства) порядок — по
        // записи. Семья — только НАСТОЯЩЕЕ сокращение со своими длинными
        // свойствами. По одному лишь общему началу судить нельзя:
        // `overflow-wrap` не часть `overflow`, и перестановка этой пары
        // ломает `textarea-pre-wrap-*`. Список расширять по одному имени.
        // Проверено: `border` и `grid` в списке (с исключениями
        // `border-spacing`/`border-collapse`/`border-radius`/`border-image`
        // и `grid-gap`) ничего не меняют — красные пары, где длинное
        // `border-*`/`grid-*` стоит ПЕРЕД сокращением, держат другие корни.
        const SHORTHANDS: &[&str] = &["background", "border-image"];
        let family = |k: &str| -> String {
            if font_members::contains(k) && d.contains("font") {
                return "font".into();
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
                && EDGE[..4].iter().any(|s| d.contains(s))
                && EDGE[4..].iter().any(|s| d.contains(s))
            {
                return "border-edge".into();
            }
            for root in SHORTHANDS {
                if k.len() > root.len()
                    && k.starts_with(root)
                    && k.as_bytes().get(root.len()) == Some(&b'-')
                    && d.contains(root)
                {
                    return (*root).into();
                }
            }
            k.into()
        };
        let place = |k: &str| names.iter().position(|n| *n == k).unwrap_or(usize::MAX);
        let mut ordered: Vec<&str> = names.clone();
        ordered.sort_by_cached_key(|k| {
            let root = family(k);
            (root.matches('-').count(), root, place(k), *k)
        });
        // Объявления свойства `k` нужной важности в порядке записи.
        let parts_of = |k: &str| -> Vec<&Declaration> {
            d.iter()
                .filter(|decl| decl.name == k && decl.important == important)
                .collect()
        };
        // `all: revert` / `all: revert-layer` (css-cascade-5 §3.2 «all»,
        // §7.2): откат КАЖДОГО свойства, кроме `direction` и `unicode-bidi`,
        // тем же правилом, что откат одного свойства ниже, — всё, что этот
        // блок сказал о свойстве ДО `all`, снимается. Ключ `all` не
        // разбирался вовсе, и `background-color: red; border-color: red;
        // all: revert` оставлял красные поля ввода (`appearance-revert-001`).
        // Порядок — по первому появлению свойства: повтор свойства ПОСЛЕ
        // `all` в том же блоке этим не различается (редкость).
        let last_all = parts_of("all").last().map(|decl| decl.value.trim());
        let all_at = last_all
            .filter(|v| matches!(*v, "revert" | "revert-layer" | "initial" | "unset"))
            .map(|_| place("all"));
        // `all: initial` / `all: unset` (css-cascade-5 §3.2) сбрасывает ВСЁ,
        // что каскад сказал до него, — не только в этом блоке, но и в ранних
        // правилах и в таблице агента: `div` становится строчным, цвет и
        // шрифт — начальными (`initial`) или наследуемыми (`unset`). Кроме
        // `direction` и `unicode-bidi`. Прежде ключ `all` понимал только
        // откат, и `.test { all: initial }` оставлял красную рамку, фон и
        // флоат раннего правила (`all-prop-001/002`). Сброс — СВОЙ шаг
        // прохода: важные объявления ранних правил применяются позже и
        // переживают его, как велит §6.1.
        if let Some(v) = last_all.filter(|v| matches!(*v, "initial" | "unset")) {
            self.reset_for_all(v == "initial", vcx);
        }
        for k in ordered {
            if let Some(at) = all_at
                && place(k) < at
                && !matches!(k, "direction" | "unicode-bidi")
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
            let parts = parts_of(k);
            // САМО слово отката по-прежнему уходит в `apply_one`: для 33
            // свойств из `initial_value()` он значит сброс к начальному, и
            // трогать это поведение здесь незачем.
            let from = parts
                .iter()
                .rposition(|decl| matches!(decl.value.trim(), "revert" | "revert-layer"))
                .unwrap_or(0);
            for decl in &parts[from..] {
                // Типизированный `attr()` подставляется тем же шагом, что и
                // `var()` (css-values-5 §7.7): после него значение разбирается
                // как обычное.
                let resolved = resolve_sibling(
                    resolve_attrs(k, &resolve_vars(&decl.value, vars), cx, vcx),
                    cx,
                );
                // CSS Variables §3: invalid after substitution means unset;
                // a preceding specified color cannot survive the computed value.
                if k == "color"
                    && crate::style::css::variable_values::has_var(&decl.value)
                    && Color::parse(&resolved, vcx).is_none()
                    && !matches!(
                        resolved.trim().to_ascii_lowercase().as_str(),
                        "inherit" | "initial" | "unset" | "revert" | "revert-layer"
                    )
                {
                    self.apply_one(k, "unset", vcx);
                } else {
                    self.apply_one(k, &resolved, vcx);
                }
            }
        }
    }
}
