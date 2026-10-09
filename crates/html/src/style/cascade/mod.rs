//! Каскад: разрешение, применение объявлений, слои.
// owner: B

pub mod defaults;
pub mod inherit;
pub mod vars;

use crate::style::cascade::defaults::{inherited_property, initial_value};
use crate::style::cascade::vars::{resolve_attrs, resolve_sibling, resolve_vars};
use crate::style::computed::{
    BG_LIST_KEYS, Computed, background_layers, font_members, top_level_comma,
};
use crate::style::css::{Decls, Rule};
use crate::style::values::value::Color;

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
            let keep = (
                self.rtl,
                self.bidi_override,
                self.bidi_isolate,
                self.bidi_plaintext,
                self.bidi_embed,
                self.bidi_inherit,
                self.decl_seq,
            );
            *self = Computed::default();
            (
                self.rtl,
                self.bidi_override,
                self.bidi_isolate,
                self.bidi_plaintext,
                self.bidi_embed,
                self.bidi_inherit,
                self.decl_seq,
            ) = keep;
            self.apply_one("display", "inline");
            if initial {
                // Наследуемые свойства: пустое поле у нас значит «от
                // родителя», поэтому начальное значение ставится явно.
                for key in [
                    "color",
                    "font-family",
                    "font-size",
                    "font-style",
                    "font-variant",
                    "font-weight",
                    "letter-spacing",
                    "line-height",
                    "list-style-position",
                    "list-style-type",
                    "quotes",
                    "text-align",
                    "text-indent",
                    "text-transform",
                    "visibility",
                    "white-space",
                    "word-spacing",
                ] {
                    if let Some(start) = initial_value(key) {
                        self.apply_one(key, start);
                    }
                }
            }
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

/// Помечено ли объявление как важное.
/// Снять с копий правил объявления, откатанные `revert-layer`
/// (css-cascade-5 §revert-layer): «as if no rules were specified in the
/// current cascade layer — or between its normal and important levels».
/// Обычный откат в слое L снимает обычные объявления свойства в L; важный —
/// важные в L и в слоях после него (у важных они слабее) и обычные в L и
/// после него. `all: revert-layer` откатывает каждое свойство, чьё
/// объявление в том же слое стоит до него. `None` — откатывать нечего.
fn revert_layers(matched: &[&crate::style::css::Rule]) -> Option<Vec<crate::style::css::Rule>> {
    use crate::style::css::DECL_SEP;
    let is_rl = |part: &str| {
        strip_important(part)
            .trim()
            .eq_ignore_ascii_case("revert-layer")
    };
    if !matched
        .iter()
        .any(|r| r.decls.values().any(|v| v.split(DECL_SEP).any(is_rl)))
    {
        return None;
    }
    let mut rules: Vec<crate::style::css::Rule> = matched.iter().map(|r| (*r).clone()).collect();
    // Порядки каскада: обычный — по возрастанию, важный — слой по убыванию.
    let normal_key =
        |r: &crate::style::css::Rule| (r.origin, r.layer.clone(), r.sel.specificity(), r.order);
    let mut keys: Vec<String> = rules
        .iter()
        .flat_map(|r| r.decls.keys().cloned())
        .filter(|k| !k.starts_with("--") && k != crate::style::css::ORDER_KEY)
        .collect();
    keys.sort();
    keys.dedup();
    // Снять части свойства `key` важности `imp` у правил, прошедших фильтр.
    let strip = |rules: &mut Vec<crate::style::css::Rule>,
                 key: &str,
                 imp: bool,
                 keep: &dyn Fn(&crate::style::css::Rule) -> bool| {
        for r in rules.iter_mut().filter(|r| !keep(r)) {
            if let Some(v) = r.decls.get(key) {
                let rest: Vec<&str> = v
                    .split(DECL_SEP)
                    .filter(|p| is_important(p) != imp)
                    .collect();
                if rest.is_empty() {
                    r.decls.remove(key);
                } else {
                    let joined = rest.join(&DECL_SEP.to_string());
                    r.decls.insert(key.to_string(), joined);
                }
            }
        }
    };
    // Победитель свойства: (индекс правила, слой, значение) по порядку каскада.
    let winner = |rules: &Vec<crate::style::css::Rule>,
                  key: &str,
                  imp: bool|
     -> Option<(Vec<u32>, String)> {
        let mut best: Option<(&crate::style::css::Rule, String)> = None;
        for r in rules {
            let Some(v) = r.decls.get(key) else { continue };
            let Some(part) = v.split(DECL_SEP).rfind(|p| is_important(p) == imp) else {
                continue;
            };
            let better = match &best {
                None => true,
                Some((b, _)) if imp => {
                    (
                        std::cmp::Reverse(r.origin),
                        std::cmp::Reverse(&r.layer),
                        r.sel.specificity(),
                        r.order,
                    ) >= (
                        std::cmp::Reverse(b.origin),
                        std::cmp::Reverse(&b.layer),
                        b.sel.specificity(),
                        b.order,
                    )
                }
                Some((b, _)) => normal_key(r) >= normal_key(b),
            };
            if better {
                best = Some((r, part.to_string()));
            }
        }
        best.map(|(r, v)| (r.layer.clone(), v))
    };
    // `all: revert-layer` (обычный): каждое свойство слоя, объявленное в
    // правиле НЕ позже правила с `all`, снимается в этом слое.
    let alls: Vec<(Vec<u32>, (u8, Vec<u32>, (u32, u32, u32), usize))> = rules
        .iter()
        .filter(|r| {
            r.decls.get("all").is_some_and(|v| {
                v.split(DECL_SEP)
                    .rfind(|p| !is_important(p))
                    .is_some_and(is_rl)
            })
        })
        .map(|r| (r.layer.clone(), normal_key(r)))
        .collect();
    for (layer, at) in alls {
        for r in rules
            .iter_mut()
            .filter(|r| r.layer == layer && normal_key(r) <= at)
        {
            let props: Vec<String> = r
                .decls
                .keys()
                .filter(|k| {
                    !k.starts_with("--")
                        && *k != crate::style::css::ORDER_KEY
                        && *k != "direction"
                        && *k != "unicode-bidi"
                })
                .cloned()
                .collect();
            for k in props {
                if let Some(v) = r.decls.get(&k) {
                    let rest: Vec<&str> = v.split(DECL_SEP).filter(|p| is_important(p)).collect();
                    if rest.is_empty() {
                        r.decls.remove(&k);
                    } else {
                        let joined = rest.join(&DECL_SEP.to_string());
                        r.decls.insert(k, joined);
                    }
                }
            }
        }
    }
    for key in keys {
        if key == "direction" || key == "unicode-bidi" || key == "all" {
            continue;
        }
        for _ in 0..16 {
            if let Some((layer, v)) = winner(&rules, &key, true) {
                if is_rl(&v) {
                    let l = layer.clone();
                    strip(&mut rules, &key, true, &|r| r.layer < l);
                    let l = layer.clone();
                    strip(&mut rules, &key, false, &|r| r.layer < l);
                    continue;
                }
                break;
            }
            if let Some((layer, v)) = winner(&rules, &key, false)
                && is_rl(&v)
            {
                let l = layer.clone();
                strip(&mut rules, &key, false, &|r| r.layer != l);
                continue;
            }
            break;
        }
    }
    Some(rules)
}

fn is_important(v: &str) -> bool {
    v.to_ascii_lowercase()
        .replace(' ', "")
        .ends_with("!important")
}

/// Значение без пометки важности; пробел перед `!` тоже допустим.
fn strip_important(v: &str) -> &str {
    match v.to_ascii_lowercase().rfind('!') {
        Some(at) if v[at..].to_ascii_lowercase().replace(' ', "") == "!important" => v[..at].trim(),
        _ => v,
    }
}
