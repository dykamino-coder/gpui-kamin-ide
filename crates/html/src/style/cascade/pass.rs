//! Computed::apply_map: применение победителей карты каскада ([`CascadeMap`])
//! с подстановкой var()/attr(), `all` и порядком сокращений и их длинных
//! свойств внутри одного блока. Длинные свойства реестра приходят
//! проверенными разбором; их значение с подстановкой разбирается здесь.

use super::map::{CascadeMap, Pick, Picks};
use super::*;
use crate::document::state::ValueCx;
use crate::style::computed::ParseResult;
use crate::style::css::{CssWide, DeclBlock, LonghandId, Origin, Specified, VarMap};
mod all_reset;
mod longhand;

impl Computed {
    /// Один блок объявлений без словаря переменных — тем же каскадом, что и
    /// правила ([`Self::apply_decls_with_vars`]).
    pub fn apply_decls(&mut self, d: &DeclBlock, vcx: ValueCx<'_>) {
        self.apply_decls_with_vars(d, &VarMap::default(), vcx);
    }

    /// То же, но со словарём переменных: `var(--x)` подставляется значением.
    ///
    /// Без этого современные темы не работают вовсе — они целиком построены на
    /// переменных, и каждое такое объявление молча терялось.
    pub fn apply_decls_with_vars(&mut self, d: &DeclBlock, vars: &VarMap, vcx: ValueCx<'_>) {
        // Один блок — один источник: важное перекрывает обычное (ключ
        // каскада), повтор свойства — по месту в блоке.
        let map = CascadeMap::single(d, Origin::Author);
        self.apply_map(&map, vars, CascadeCx::NONE, vcx);
    }

    /// Применить победителей карты каскада. Каждое имя свойства получает ОДНО
    /// значение — победителя (css-cascade-5 §6.1); негодный при применении
    /// победитель уступает следующему кандидату, как если бы его объявление
    /// было отброшено при разборе (CSS 2.1 §4.2).
    pub(crate) fn apply_map(
        &mut self,
        map: &CascadeMap<'_>,
        vars: &VarMap,
        cx: CascadeCx,
        vcx: ValueCx<'_>,
    ) {
        // Каждое применение карты — новый проход каскада по стилю: слой
        // (`:hover`, кадр анимации) ложится поверх уже применённого.
        self.decl_order.pass += 1;
        // `all: initial | unset` (§3.2) задаёт и свойства, которых не назвало
        // ни одно объявление: сброс идёт ДО победителей — их выбор уже учёл
        // `all` кандидатом каждого имени.
        let mut reset = None;
        if let Some(Pick::Value(c, v)) = map.all_picks().next()
            && matches!(v.to_ascii_lowercase().as_str(), "initial" | "unset")
        {
            self.reset_for_all(v.eq_ignore_ascii_case("initial"), vcx);
            reset = Some(c.decl);
        }
        let mut winners: Vec<(&str, Pick<'_>, Picks<'_, '_>)> = map
            .names()
            .iter()
            .filter(|k| !k.starts_with("--"))
            .filter_map(|&k| {
                let mut picks = map.picks(k);
                let first = picks.next()?;
                // Имя, где победил тот самый `all`, уже задано сбросом.
                if let Pick::Value(c, _) = first
                    && reset.is_some_and(|r| std::ptr::eq(r, c.decl))
                {
                    return None;
                }
                Some((k, first, picks))
            })
            .collect();
        // Порядок применения: по ключу победителя, а внутри одного блока —
        // семьями. Порядок ЗАПИСИ решает только между СОКРАЩЕНИЕМ и его
        // длинным свойством (`background` и `background-color`, `border` и
        // `border-width`): `background-color: red; background: green`
        // обязано дать зелёный, а обратная запись красный.
        //
        // Между СОСЕДЯМИ (`border-width` и `border-style`, `white-space` и
        // `overflow-wrap`) порядок записи НЕ применяется: наши свойства
        // кое-где читают состояние друг друга на применении (`border-style`
        // пишет ширину), и полный порядок вскрывает эту зависимость
        // (`border-width-012`, `textarea-pre-wrap-*`). Семьи идут «сперва
        // общее»: общность меряется числом дефисов (`border` <
        // `border-width` < `border-top-width`), так `border: solid gray;
        // border-width: 1px 2px 3px 4px` даёт 1/2/3/4. Порядок уходит вместе
        // с раскрытием сокращений при разборе.
        winners.sort_by_cached_key(|(k, pick, _)| {
            let (block, pos) = match pick {
                Pick::Value(c, _) => (Some(c.block), c.key.decl_index()),
                Pick::Unset(key) => (None, key.decl_index()),
            };
            let root = family(k, block);
            (
                pick.key().block_key(),
                root.matches('-').count(),
                root,
                pos,
                k.to_string(),
            )
        });
        for (k, first, rest) in winners {
            self.apply_winner(k, first, rest, vars, cx, vcx);
        }
        // Прямая запись после карты — позже любого её объявления.
        self.decl_order.pass += 1;
        self.decl_order.key = Default::default();
    }

    fn apply_winner(
        &mut self,
        k: &str,
        first: Pick<'_>,
        mut rest: Picks<'_, '_>,
        vars: &VarMap,
        cx: CascadeCx,
        vcx: ValueCx<'_>,
    ) {
        let mut pick = first;
        loop {
            self.decl_order.key = pick.key();
            let (cand, value) = match pick {
                Pick::Value(c, v) => (c, v),
                // Откат к агенту, у которого правила для имени нет: значение
                // агента — умолчание стиля. Часть умолчаний агента у нас не
                // правила листа, а неявное значение по тегу (`div` блочный,
                // поле ввода с родной рамкой), и `unset` стёр бы его
                // (`revert-val-001`, `appearance-revert-001`).
                Pick::Unset(_) => return,
            };
            // Длинное свойство реестра проверено разбором: негодное значение
            // в каскад не попало, и запасной кандидат не нужен.
            if let Some(id) = LonghandId::from_name(k) {
                return self.apply_longhand(id, cand, value, vars, cx, vcx);
            }
            // Значение с `var()` годно при разборе (CSS Variables §3):
            // негодная подстановка — `unset`, а не прежний кандидат.
            let pending = matches!(cand.decl.specified, Specified::Pending(_));
            let wide = matches!(cand.decl.specified, Specified::Wide(_));
            let fallback = !pending && !wide && rest.clone().next().is_some();
            if fallback {
                self.declaration_parse = ParseResult::Invalid;
            }
            self.apply_resolved(k, value, vars, cx, vcx);
            if !fallback {
                return;
            }
            let accepted = self.declaration_parse == ParseResult::Accepted;
            self.declaration_parse = ParseResult::Inactive;
            if accepted {
                return;
            }
            match rest.next() {
                Some(next) => pick = next,
                None => return,
            }
        }
    }

    fn apply_resolved(
        &mut self,
        k: &str,
        value: &str,
        vars: &VarMap,
        cx: CascadeCx,
        vcx: ValueCx<'_>,
    ) {
        // Типизированный `attr()` подставляется тем же шагом, что и `var()`
        // (css-values-5 §7.7): после него значение разбирается как обычное.
        let resolved = resolve_sibling(resolve_attrs(k, &resolve_vars(value, vars), cx, vcx), cx);
        // CSS Variables §3: invalid after substitution means unset;
        // a preceding specified color cannot survive the computed value.
        if k == "color"
            && crate::style::css::variable_values::has_var(value)
            && Color::parse(&resolved, vcx).is_none()
            && !is_wide(&resolved)
        {
            self.apply_one(k, "unset", vcx);
        } else {
            self.apply_one(k, &resolved, vcx);
        }
    }
}

fn is_wide(v: &str) -> bool {
    CssWide::parse(v).is_some()
}

/// Семья имени внутри блока: НАСТОЯЩЕЕ сокращение со своими длинными
/// свойствами. По одному лишь общему началу судить нельзя: `overflow-wrap` не
/// часть `overflow`, и перестановка этой пары ломает `textarea-pre-wrap-*`.
fn family(k: &str, block: Option<&DeclBlock>) -> String {
    let Some(d) = block else {
        return k.into();
    };
    if font_members::contains(k) && d.contains("font") {
        return "font".into();
    }
    // A side shorthand (`border-right: 12px solid`) resets that side's color
    // to `currentColor`; a later `border-color: pink` must win over it
    // (css-cascade-4 §6.4: order of appearance; css-gaps
    // `grid-gap-decorations-*-ref`). Only side shorthands and the three
    // all-side shorthands share the order — `border-width`/`border-style`
    // among themselves keep the name order (see the note in `apply_map`).
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
    const SHORTHANDS: &[&str] = &["background", "border-image"];
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
}
