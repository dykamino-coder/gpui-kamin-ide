//! Каскад: разрешение, применение объявлений, слои.
// owner: B

use crate::style::cascade::defaults::{inherited_property, initial_value};
use crate::style::cascade::vars::{resolve_attrs, resolve_sibling, resolve_vars};
use crate::style::computed::{
    BG_LIST_KEYS, Computed, background_layers, font_members, top_level_comma,
};
use crate::style::css::{Decls, Rule};
use crate::style::values::value::Color;

pub mod defaults;
pub mod inherit;
mod pass;
mod revert;
pub mod vars;
use revert::{is_important, revert_layers, strip_important};

impl Computed {
    /// Собрать стиль узла: правила таблицы (по специфичности), затем `style=""`.
    pub fn resolve(matched: &mut Vec<&Rule>, inline: &Decls) -> Computed {
        Computed::resolve_with_vars(matched, inline, &Decls::new())
    }

    /// То же с переменными темы.
    pub fn resolve_with_vars(matched: &mut Vec<&Rule>, inline: &Decls, vars: &Decls) -> Computed {
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
        // `revert-layer` (css-cascade-5 §revert-layer) решается ДО прохода:
        // объявления откатываемого слоя (а у важного — и всё между его
        // обычным и важным уровнями) снимаются с копий правил.
        let reverted = revert_layers(matched);
        let owned_refs: Vec<&crate::style::css::Rule> = reverted.iter().flatten().collect();
        let matched: &mut Vec<&crate::style::css::Rule> = &mut if reverted.is_some() {
            owned_refs
        } else {
            matched.clone()
        };
        let mut c = Computed::default();
        // Два прохода по ВСЕМУ каскаду, а не внутри каждого правила: важность
        // — самый старший ключ сравнения (CSS Cascade §6.1), поэтому важное
        // объявление раннего правила обязано пережить обычное объявление
        // позднего. Пока проходы шли внутри правила, `!important` действовал
        // только против соседей по тому же блоку.
        for rule in matched.iter() {
            c.apply_pass(&rule.decls, vars, false);
        }
        c.apply_pass(inline, vars, false);
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
            c.apply_pass(&rule.decls, vars, true);
        }
        c.apply_pass(inline, vars, true);
        // `currentColor` в рамке и фоне значит «цвет текста этого элемента» —
        // подставляем уже после того, как цвет стал известен.
        if c.border_color_is_current {
            c.border_color = c.color;
        }

        // Окраска фильтром здесь НЕ делается: результат оседал в
        // долгоживущем стиле узла, и покадровая окраска в `inline::inherit`
        // применяла фильтр ВТОРОЙ раз (grayscale темнил вдвое). Единственная
        // точка окраски — слияние при отрисовке.
        c
    }

    pub fn apply_decls(&mut self, d: &Decls) {
        for (k, v) in d {
            if k.starts_with(crate::style::css::CUSTOM_IMPORTANT) {
                continue;
            }
            for part in v.split(crate::style::css::DECL_SEP) {
                self.apply_one(k, part);
            }
        }
    }

    /// То же, но со словарём переменных: `var(--x)` подставляется значением.
    ///
    /// Без этого современные темы не работают вовсе — они целиком построены на
    /// переменных, и каждое такое объявление молча терялось.
    pub fn apply_decls_with_vars(&mut self, d: &Decls, vars: &Decls) {
        // Два прохода: сначала обычные объявления, затем помеченные
        // `!important`. Так важное перекрывает любое обычное независимо от
        // порядка правил — раньше пометка просто срезалась, и объявление
        // конкурировало на общих основаниях.
        // Порядок объявлений внутри правила: сперва СОКРАЩЁННЫЕ, потом
        // отдельные. `border: solid gray; border-width: 1px 2px 3px 4px`
        // обязано дать 1/2/3/4, а не 3px от сокращения; словарь объявлений
        // исходного порядка не помнит, и без сортировки исход зависел от
        // случайного обхода хеш-таблицы — от запуска к запуску РАЗНОГО
        // (`abs-pos-border-offset-001`). Общность меряется числом дефисов:
        // `border` < `border-width` < `border-top-width`.
        for important in [false, true] {
            self.apply_pass(d, vars, important);
        }
    }

    // `pub(crate)`: `motion` синтезирует строку `transform` и кормит её тем же
    // разборщиком — отдельного конвейера под offset-трансформ нет.
    pub(crate) fn apply_one(&mut self, key: &str, val: &str) {
        self.decl_seq += 1;
        let v = val.trim();
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
            return self.apply_one(key, "inherit");
        }
        if matches!(v, "initial" | "unset" | "revert" | "revert-layer")
            && let Some(start) = initial_value(key)
        {
            return self.apply_one(key, start);
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
            return self.apply_one(key, start);
        }
        // Фон — СПИСОК слоёв (css-backgrounds-3 §2.1: «comma-separated list
        // of values … the first value represents the top layer»). Одиночные
        // поля стиля несут верхний слой, а список целиком хранится сырым —
        // по нему рисуются все слои (`Computed::bg_layers`). Запись без
        // запятой список своего свойства снимает; сокращение — все.
        if BG_LIST_KEYS.contains(&key) {
            if key == "background" {
                self.bg_lists.clear();
            } else {
                self.bg_lists.retain(|(k, _)| k != key);
            }
            if top_level_comma(v).is_some() {
                if key != "background" {
                    // Верхний слой — обычным разбором; список кладётся ПОСЛЕ:
                    // вложенный вызов того же свойства его бы снял.
                    let first = background_layers(v)[0].to_string();
                    self.apply_one(key, &first);
                    self.bg_lists.push((key.to_string(), v.to_string()));
                    return;
                }
                self.bg_lists.push((key.to_string(), v.to_string()));
            }
        }
        // Свойства разнесены по группам (`props/*`): у каждого ключа ровно одна группа,
        // ветви внутри группы идут в исходном порядке.
        let groups: [fn(&mut Self, &str, &str, &str, &mut bool); 13] = [
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
            group(self, key, val, v, &mut hit);
            if hit {
                return;
            }
        }
    }
}
