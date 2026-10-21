//! Каскад: разрешение, применение объявлений, слои.

use crate::document::state::ValueCx;

use crate::style::cascade::defaults::{inherited_property, initial_value};
use crate::style::cascade::vars::{CascadeCx, resolve_attrs, resolve_sibling, resolve_vars};
use crate::style::computed::{
    BgListProp, Computed, background_layers, font_members, top_level_comma,
};
use crate::style::css::{DeclBlock, Rule, VarMap};
use crate::style::values::value::Color;

mod color_initial;
pub mod defaults;
pub mod inherit;
pub(crate) mod inherited_value;
mod inline_rollback;
mod pass;
mod revert;
pub mod vars;
use revert::revert_layers;

impl Computed {
    /// Собрать стиль узла: правила таблицы (по специфичности), затем `style=""`.
    pub fn resolve(matched: &mut Vec<&Rule>, inline: &DeclBlock, vcx: ValueCx<'_>) -> Computed {
        Computed::resolve_with_vars(matched, inline, &VarMap::new(), vcx)
    }

    /// То же с переменными темы.
    pub fn resolve_with_vars(
        matched: &mut Vec<&Rule>,
        inline: &DeclBlock,
        vars: &VarMap,
        vcx: ValueCx<'_>,
    ) -> Computed {
        Computed::resolve_in(matched, inline, vars, CascadeCx::NONE, vcx)
    }

    /// То же для элемента `cx`: его атрибуты и место среди братьев видят
    /// `attr()` и `sibling-index()`.
    pub fn resolve_in(
        matched: &mut Vec<&Rule>,
        inline: &DeclBlock,
        vars: &VarMap,
        cx: CascadeCx,
        vcx: ValueCx<'_>,
    ) -> Computed {
        // Слой старше специфичности (css-cascade-5 §6.4): у обычных
        // объявлений поздний слой сильнее, у важных — ранний.
        matched.sort_by(|a, b| {
            (a.origin, &a.layer, a.sel.specificity(), a.order).cmp(&(
                b.origin,
                &b.layer,
                b.sel.specificity(),
                b.order,
            ))
        });
        let inline_rollback = inline_rollback::prepare(matched, inline);
        let inline = inline_rollback.as_ref().map_or(inline, |(_, d)| d);
        let rollback_refs: Vec<&Rule> = inline_rollback
            .iter()
            .flat_map(|(rules, _)| rules.iter())
            .collect();
        let matched = if inline_rollback.is_some() {
            rollback_refs.as_slice()
        } else {
            matched.as_slice()
        };
        // `revert-layer` (css-cascade-5 §revert-layer) решается ДО прохода:
        // объявления откатываемого слоя (а у важного — и всё между его
        // обычным и важным уровнями) снимаются с копий правил.
        let reverted = revert_layers(matched);
        let owned_refs: Vec<&crate::style::css::Rule> = reverted.iter().flatten().collect();
        let matched: &mut Vec<&crate::style::css::Rule> = &mut if reverted.is_some() {
            owned_refs
        } else {
            matched.to_vec()
        };
        let mut c = Computed::default();
        // Два прохода по ВСЕМУ каскаду, а не внутри каждого правила: важность
        // — самый старший ключ сравнения (CSS Cascade §6.1), поэтому важное
        // объявление раннего правила обязано пережить обычное объявление
        // позднего. Пока проходы шли внутри правила, `!important` действовал
        // только против соседей по тому же блоку.
        for rule in matched.iter() {
            c.apply_pass(&rule.decls, vars, false, cx, vcx);
        }
        c.apply_pass(inline, vars, false, cx, vcx);
        // Важные идут в ОБРАТНОМ порядке происхождений: важное правило агента
        // старше важного авторского (§6.4.4), поэтому применяется последним.
        let mut important: Vec<&&Rule> = matched.iter().collect();
        important.sort_by(|a, b| {
            (
                std::cmp::Reverse(a.origin),
                std::cmp::Reverse(&a.layer),
                a.sel.specificity(),
                a.order,
            )
                .cmp(&(
                    std::cmp::Reverse(b.origin),
                    std::cmp::Reverse(&b.layer),
                    b.sel.specificity(),
                    b.order,
                ))
        });
        for rule in important {
            c.apply_pass(&rule.decls, vars, true, cx, vcx);
        }
        c.apply_pass(inline, vars, true, cx, vcx);
        // `currentColor` в рамке и фоне значит «цвет текста этого элемента» —
        // подставляем уже после того, как цвет стал известен.
        if c.border_color_is_current {
            c.border_color = c.color;
        }

        // Окраска фильтром здесь НЕ делается: результат оседал в
        // долгоживущем стиле узла, и покадровая окраска в `style::cascade::inherit`
        // применяла фильтр ВТОРОЙ раз (grayscale темнил вдвое). Единственная
        // точка окраски — слияние при отрисовке.
        c
    }

    // `pub(crate)`: `motion` синтезирует строку `transform` и кормит её тем же
    // разборщиком — отдельного конвейера под offset-трансформ нет.
    pub(crate) fn apply_one(&mut self, key: &str, val: &str, vcx: ValueCx<'_>) {
        self.decl_seq += 1;
        let v = val.trim();
        inherit::note_flow_align(self, key, v);
        if color_initial::apply(self, key, v, vcx) {
            return;
        }
        // Общие для всех свойств слова `initial`/`unset`/`revert`. Для
        // НАСЛЕДУЕМОГО свойства это не «оставить как есть»: незаданное поле у
        // нас берётся от родителя, поэтому такое объявление молча наследовало
        // вместо сброса — на `static-position` отступ первой строки уходил в
        // абсолютный блок, и красное проступало из-под него.
        // `unset` у НАСЛЕДУЕМОГО свойства — это `inherit`, у ненаследуемого —
        // `initial` (css-cascade-4 §7.3.3). Прежде любое `unset` шло в
        // начальное значение, и `color: unset` давал чёрный вместо цвета
        // родителя (`unset-val-001`).
        if v == "unset" && inherited_property(key) {
            return self.apply_one(key, "inherit", vcx);
        }
        if matches!(v, "initial" | "unset" | "revert" | "revert-layer")
            && let Some(start) = initial_value(key)
        {
            return self.apply_one(key, start, vcx);
        }
        // Начальные значения, которые годятся ТОЛЬКО для `initial`/`unset`:
        // `revert` у автора откатывает к таблице агента, а там у `div`
        // `display: block`, не начальное `inline`.
        if matches!(v, "initial" | "unset")
            && let Some(start) = match key {
                "background-color" => Some("transparent"),
                "background-image" => Some("none"),
                "display" => Some("inline"),
                "float" => Some("none"),
                "position" => Some("static"),
                "opacity" => Some("1"),
                _ => None,
            }
        {
            return self.apply_one(key, start, vcx);
        }
        if inherited_value::note(self, key, v, vcx) {
            return;
        }
        // Фон — СПИСОК слоёв (css-backgrounds-3 §2.1: «comma-separated list
        // of values … the first value represents the top layer»). Одиночные
        // поля стиля несут верхний слой, а список целиком хранится сырым —
        // по нему рисуются все слои (`Computed::bg_layers`). Запись без
        // запятой список своего свойства снимает; сокращение — все.
        if let Some(prop) = BgListProp::from_key(key) {
            if prop == BgListProp::BlendMode && !crate::style::computed::background_blend_valid(v) {
                return;
            }
            if prop == BgListProp::Shorthand {
                // background-blend-mode is not a background shorthand member.
                self.bg_lists.retain(|k| k == BgListProp::BlendMode);
            } else {
                self.bg_lists.remove(prop);
            }
            if top_level_comma(v).is_some() {
                if prop != BgListProp::Shorthand {
                    // Верхний слой — обычным разбором; список кладётся ПОСЛЕ:
                    // вложенный вызов того же свойства его бы снял.
                    let first = background_layers(v)[0].to_string();
                    self.apply_one(key, &first, vcx);
                    self.bg_lists.set(prop, v.to_string(), vcx);
                    return;
                }
                self.bg_lists.set(prop, v.to_string(), vcx);
            }
        }
        self.apply_property(key, v, vcx);
    }

    /// Query the actual property dispatcher, including delegated property groups.
    pub(crate) fn recognizes_property(key: &str, vcx: ValueCx<'_>) -> bool {
        Self::default().apply_property(key, "", vcx)
    }

    // The temporary cascade callback borrows the current property and variable state.
    #[allow(clippy::type_complexity)]
    fn apply_property(&mut self, key: &str, v: &str, vcx: ValueCx<'_>) -> bool {
        let groups: [fn(&mut Self, &str, &str, &mut bool, ValueCx<'_>); 14] = [
            Self::apply_scroll,
            Self::apply_display_flex,
            Self::apply_grid,
            Self::apply_box_model,
            Self::apply_border,
            Self::apply_position,
            Self::apply_background,
            Self::apply_mask_clip,
            Self::apply_font,
            Self::apply_text,
            Self::apply_text_decor,
            Self::apply_multicol,
            Self::apply_transform,
            Self::apply_effects,
        ];
        for group in groups {
            let mut hit = true;
            group(self, key, v, &mut hit, vcx);
            if hit {
                return true;
            }
        }
        false
    }
}
