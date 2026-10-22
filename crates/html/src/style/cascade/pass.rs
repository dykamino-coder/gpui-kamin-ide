//! Computed::apply_map: применение победителей карты каскада ([`CascadeMap`])
//! по фазам вычисления ([`Phase`]) с подстановкой var()/attr(), `all` и
//! порядком сокращений и их длинных свойств внутри одного блока. Итоги
//! ранних фаз доходят до поздних явным входом [`ComputeCx`]. Длинные
//! свойства реестра приходят проверенными разбором; их значение с
//! подстановкой разбирается здесь.

use super::compute_cx::ComputeCx;
use super::map::{CascadeMap, Pick, Picks};
use super::phase::Phase;
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
        let mut winners: Vec<(Phase, &str, Pick<'_>, Picks<'_, '_>)> = map
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
                Some((Phase::of(k), k, first, picks))
            })
            .collect();
        // Порядок применения: по фазе вычисления (свойства, от которых
        // зависят другие, — раньше; `phase`), затем по ключу победителя, а
        // внутри одного блока —
        // семьями. Порядок ЗАПИСИ решает только между СОКРАЩЕНИЕМ вне
        // реестра и его длинным свойством (`background` и `background-color`):
        // `background-color: red; background: green` обязано дать зелёный, а
        // обратная запись красный. Сокращения реестра (`margin`, `border`,
        // `outline`) раскрыты разбором, и их длинные свойства пишут каждое
        // своё поле целиком — порядок между ними не важен.
        //
        // Между СОСЕДЯМИ вне реестра (`white-space` и `overflow-wrap`)
        // порядок записи НЕ применяется: эти свойства кое-где читают
        // состояние друг друга на применении, и полный порядок вскрывает эту
        // зависимость (`textarea-pre-wrap-*`). Семьи идут «сперва общее»:
        // общность меряется числом дефисов. Порядок уходит вместе с
        // раскрытием остальных сокращений при разборе.
        winners.sort_by_cached_key(|(phase, k, pick, _)| {
            let (block, pos) = match pick {
                Pick::Value(c, _) => (Some(c.block), c.key.decl_index()),
                Pick::Unset(key) => (None, key.decl_index()),
            };
            let root = family(k, block);
            (
                *phase,
                pick.key().block_key(),
                root.matches('-').count(),
                root,
                pos,
                k.to_string(),
            )
        });
        let mut compute = ComputeCx::new(cx, vcx);
        let mut winners = winners.into_iter().peekable();
        for phase in Phase::ALL {
            compute.before(phase);
            while let Some((_, k, first, rest)) = winners.next_if(|w| w.0 == phase) {
                self.apply_winner(k, first, rest, vars, &compute);
            }
            compute.after(phase, self);
        }
        self.settle_border_widths();
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
        compute: &ComputeCx<'_>,
    ) {
        let (cx, vcx) = (compute.element, compute.vcx);
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
            self.apply_resolved(k, value, vars, compute);
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

    fn apply_resolved(&mut self, k: &str, value: &str, vars: &VarMap, compute: &ComputeCx<'_>) {
        let (cx, vcx) = (compute.element, compute.vcx);
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
        } else if is_side_radius(k) && !is_wide(&resolved) {
            // Логическая сторона сопоставляется по письму — итогу фазы
            // письма (css-logical-1 §4), а не по полям стиля на этот миг.
            self.apply_side_radius(k, &resolved, compute.wm.rtl(), vcx);
        } else {
            self.apply_one(k, &resolved, vcx);
        }
    }
}

fn is_wide(v: &str) -> bool {
    CssWide::parse(v).is_some()
}

/// Логические сокращения радиусов стороны (css-borders-4, tentative).
fn is_side_radius(k: &str) -> bool {
    matches!(
        k,
        "border-block-start-radius"
            | "border-block-end-radius"
            | "border-inline-start-radius"
            | "border-inline-end-radius"
    )
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
