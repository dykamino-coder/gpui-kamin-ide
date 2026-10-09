//! Обход html5ever-дерева: каскад и сборка узлов (walk).

use crate::dom::*;
use crate::style::computed::{Computed, Display, Position};
use crate::style::css::{Decls, Keyframes, Rule, parse_decls};
use crate::style::select::has::has_marks_of;
use crate::style::select::matching::{matches, matches_ignoring_pseudo};
use crate::style::select::{Ancestor, Sibs, Spot, census_of};
use markup5ever_rcdom::{Handle, NodeData};
use std::collections::HashMap;
use std::rc::Rc;

/// Обойти детей узла, посчитав каждому его место среди соседей.
#[allow(clippy::too_many_arguments)]
pub(crate) fn walk_children(
    handle: &Handle,
    rules: &[Rule],
    vars: &Decls,
    frames: &HashMap<String, Keyframes>,
    counter: &mut u64,
    counters: &mut crate::style::generated::counters::Counters,
    path: &[Ancestor],
    preserve: bool,
    out: &mut Vec<Node>,
) {
    let children = handle.children.borrow();
    // Перепись братьев ЦЕЛИКОМ до обхода: `:nth-last-child(… of S)` смотрит
    // и на последующих, поэтому паспорта всех детей-элементов собираются
    // заранее, а каждый узел получает свою позицию в общем списке.
    let (spots, all) = census_of(&children);
    let all = Rc::new(all);
    let mut pos = 0usize;
    for (idx, (child, spot)) in children.iter().zip(&spots).enumerate() {
        let is_elem = spot.index != 0;
        let sibs = Sibs {
            all: &all,
            pos,
            is_elem,
            rc: Some(&all),
        };
        walk(
            child, rules, vars, frames, counter, counters, path, *spot, preserve, sibs, &children,
            &spots, idx, out,
        );
        pos += usize::from(is_elem);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn walk(
    handle: &Handle,
    rules: &[Rule],
    vars: &Decls,
    frames: &HashMap<String, Keyframes>,
    counter: &mut u64,
    counters: &mut crate::style::generated::counters::Counters,
    path: &[Ancestor],
    spot: Spot,
    preserve: bool,
    sibs: Sibs,
    level: &[Handle],
    spots: &[Spot],
    level_pos: usize,
    out: &mut Vec<Node>,
) {
    match &handle.data {
        NodeData::Text { contents } => {
            let text = contents.borrow().to_string();
            // Пустой узел ПО CSS — только схлопываемые пробелы
            // (`space`/`tab`/`CR`/`LF`). `str::trim` снимает весь юникодный
            // пробел, и узел из идеографических U+3000 отбрасывался прямо на
            // разборе: строка из них не доезжала до раскладки вовсе
            // (`trailing-ideographic-space-017`).
            let collapsible = |c: char| matches!(c, ' ' | '\t' | '\r' | '\n');
            // Под `white-space: pre*` схлопывания нет вовсе: узел из одного
            // перевода строки — это жёсткий разрыв, и выбрасывать его нельзя
            // (`word-space-transform-011`: `あ<wbr>い<wbr>\n<wbr>う` шло одной
            // строкой, потому что перевод пропадал ещё на разборе).
            // Пробельный узел без пробела (`</span>\n\t<span>`) между двумя
            // строчными соседями — это тоже схлопываемый пробел строки
            // (css-text-3 §4.1.1: перевод строки и табуляция превращаются в
            // пробел), а не отбивка разметки между блоками. Выброшенный, он
            // склеивал соседние слова: `multicol-basic-001…004` — «XXXX» двух
            // span сливались в одно слово, колонки резались не там.
            let between_inline = !preserve
                && text.chars().all(collapsible)
                && matches!(out.last(), Some(Node::Element(prev))
                    if prev.inline && prev.style.display.is_none() && prev.tag != "br")
                && level[level_pos + 1..]
                    .iter()
                    .find(|h| !matches!(h.data, NodeData::Comment { .. }))
                    .is_some_and(|h| match &h.data {
                        NodeData::Text { contents } => {
                            !contents.borrow().chars().all(collapsible)
                        }
                        NodeData::Element { name, .. } => {
                            let tag = local_name(&name.local);
                            tag != "br" && INLINE_TAGS.contains(&tag.as_str())
                        }
                        _ => false,
                    });
            let text = if between_inline { " ".to_string() } else { text };
            if preserve || !text.chars().all(collapsible) || text.contains(' ') {
                // Комментарий разрезает пробельный кусок надвое, а схлопывание
                // работает по одному узлу — выходило два пробела подряд.
                // Соседние текстовые узлы склеиваются в один отрезок.
                match out.last_mut() {
                    Some(Node::Text(prev)) if !preserve => prev.push_str(&text),
                    _ => out.push(Node::Text(text)),
                }
            }
        }
        NodeData::Element { name, attrs, .. } => {
            let tag = local_name(&name.local);
            if DROP_TAGS.contains(&tag.as_str()) {
                return;
            }
            let attrs: Vec<(String, String)> = attrs
                .borrow()
                .iter()
                .map(|a| (a.name.local.to_string(), a.value.to_string()))
                .collect();
            let id = attrs
                .iter()
                .find(|(k, _)| k == "id")
                .map(|(_, v)| v.clone());
            let classes: Vec<String> = attrs
                .iter()
                .find(|(k, _)| k == "class")
                .map(|(_, v)| v.split_whitespace().map(str::to_string).collect())
                .unwrap_or_default();
            let mut me = Ancestor {
                counter_style: Default::default(),
                tag: tag.clone(),
                html_attrs: content::html_attributes(&name.ns),
                id: id.clone(),
                classes: classes.clone(),
                attrs: attrs.clone(),
                spot,
                href: attrs
                    .iter()
                    .find(|(k, _)| k == "href")
                    .map(|(_, v)| v.clone()),
                dir: attrs.iter().find(|(k, _)| k == "dir").and_then(|(_, v)| {
                    match v.to_ascii_lowercase().as_str() {
                        "rtl" => Some(true),
                        "ltr" => Some(false),
                        _ => None,
                    }
                }),
                has_marks: has_marks_of(handle),
                featureless: None,
                slot: slot_of(handle),
                peers: sibs.is_elem.then_some(sibs.rc).flatten().map(|r| (r.clone(), sibs.pos)),
            };

            let inline_decls: Decls = attrs
                .iter()
                .find(|(k, _)| k == "style")
                .map(|(_, v)| parse_decls(v))
                .unwrap_or_default();
            let mut matched: Vec<&Rule> = rules
                .iter()
                .filter(|r| matches(&r.sel, &me, path, sibs))
                .collect();
            // Правила `:host`/`:host(S)` из ТЕНИ хоста ложатся на сам хост
            // (css-shadow-1 §3.1; Blink `MatchHostRules`). Хост для них
            // безлик: совпадает только `:host`-компаунд без предков и братьев.
            let shadow = shadow_of(handle);
            if let Some(shadow) = &shadow {
                matched.extend(
                    shadow
                        .scope
                        .rules
                        .iter()
                        .filter(|r| matches(&r.sel, &shadow.marker, &[], Sibs::EMPTY)),
                );
            }
            // Свои переменные: родительские, поверх них объявления
            // совпавших правил в порядке каскада, поверх — свои же в
            // атрибуте. Пока словарь был один на документ, `:root{--c:red}`
            // и `.dark{--c:blue}` складывались в него подряд, и последнее
            // объявление красило ВЕСЬ документ — переключение темы классом
            // не работало в принципе.
            let registered = crate::style::css::property_rules();
            let cascaded = crate::style::css::custom_properties::cascade(
                &matched, &inline_decls, vars, &registered, syntax_accepts,
            );
            let own_vars = crate::style::css::variable_values::compute(
                &cascaded, vars, &registered, syntax_accepts,
            );
            let vars = &own_vars;
            // Используемая схема цвета (css-color-adjust-1 §color-scheme-prop):
            // своё `color-scheme` — последнее по каскаду, иначе родительская
            // (свойство наследуемое). Тёмная — когда названа только `dark`:
            // при `light dark` берётся предпочтение пользователя, у стенда
            // светлое. Держится на время узла и его потомков.
            let scheme = {
                let mut by_cascade: Vec<&&Rule> = matched.iter().collect();
                by_cascade.sort_by(|a, b| {
                    (a.origin, &a.layer, a.sel.specificity(), a.order)
                        .cmp(&(b.origin, &b.layer, b.sel.specificity(), b.order))
                });
                let mut last: Option<String> = None;
                for rule in by_cascade {
                    if let Some(v) = rule.decls.get("color-scheme") {
                        last = Some(v.clone());
                    }
                }
                if let Some(v) = inline_decls.get("color-scheme") {
                    last = Some(v.clone());
                }
                last
            };
            struct SchemeGuard(bool);
            impl Drop for SchemeGuard {
                fn drop(&mut self) {
                    crate::style::values::value::set_dark_scheme(self.0);
                }
            }
            let parent_dark = crate::style::values::value::dark_scheme();
            let _scheme = SchemeGuard(parent_dark);
            if let Some(v) = scheme {
                let low = v.to_ascii_lowercase();
                let words: Vec<&str> = low.split_whitespace().collect();
                let dark = words.contains(&"dark") && !words.contains(&"light");
                if !low.contains("inherit") {
                    crate::style::values::value::set_dark_scheme(dark);
                }
            }
            // Типизированный `attr()` читает атрибуты ЭТОГО элемента
            // (css-values-5 §7.7): слот ставится только на время его каскада.
            crate::style::cascade::vars::set_current_attrs(&attrs);
            crate::style::cascade::vars::set_current_sibling((spot.index > 0).then_some((spot.index, spot.total)));
            let hints = presentational_hints::rules(&tag, &attrs);
            matched.extend(hints.iter());
            let mut style = Computed::resolve_with_vars(&mut matched, &inline_decls, vars);
            inherit_counter_decls(&mut style, path.last().map(|p| &p.counter_style));
            apply_value_hint(&mut style, &me);
            me.counter_style = counter_snapshot(&style);
            crate::style::cascade::vars::clear_current_attrs();
            crate::style::cascade::vars::set_current_sibling(None);
            // Корневые метрики для `rem`/`rlh` (css-values-4 §6.1.4).
            // Записываются ЗДЕСЬ, а не в наследовании: `Len::parse` работает
            // на разборе объявлений, а `walk` идёт в порядке документа —
            // корень разбирается раньше любого потомка, и его `25rem` уже
            // читается верно. Собственные объявления корня успевают
            // разобраться по прежней базе; на самом корне `rem` по спеке и
            // так меряется РОДИТЕЛЬСКИМИ (начальными) метриками.
            if tag == "html" {
                let font = match style.font_size {
                    Some(crate::style::values::value::Len::Px(v)) => v,
                    Some(crate::style::values::value::Len::Em(k)) | Some(crate::style::values::value::Len::Pct(k)) => k * 16.0,
                    _ => 16.0,
                };
                let family = style.font_family.clone().unwrap_or_default();
                let line = match style.line_height {
                    Some(crate::style::values::value::Len::Px(v)) => v,
                    Some(crate::style::values::value::Len::Em(k)) | Some(crate::style::values::value::Len::Pct(k)) => k * font,
                    _ => {
                        let f = crate::text::metrics::normal_line(&family);
                        if f > 0.0 { f * font } else { 1.2 * font }
                    }
                };
                crate::style::values::value::set_root_metrics(font, line);
                crate::style::values::value::set_root_font_view(match style.font_size {
                    Some(crate::style::values::value::Len::Vh(k)) => Some((true, k)),
                    Some(crate::style::values::value::Len::Vw(k)) => Some((false, k)),
                    _ => None,
                });
            }
            apply_presentational_size(&mut style, &tag, &attrs);
            promote_auto_ratio(&mut style, &tag);
            presentational_hints::colors(&mut style, &tag, &attrs);
            finish_inline_display(&mut style, &tag, &attrs);
            style.plain_block_box = {
                use crate::style::computed::Display;
                let special = matches!(
                    tag.as_str(),
                    "table" | "caption" | "colgroup" | "col" | "thead" | "tbody" | "tfoot"
                        | "tr" | "td" | "th" | "hr" | "fieldset" | "legend" | "details"
                        | "summary" | "dialog" | "option" | "optgroup" | "html" | "body"
                        | "input" | "textarea" | "select" | "button" | "img" | "video"
                        | "canvas" | "iframe" | "embed" | "object" | "svg" | "meter"
                        | "progress"
                );
                let block = match style.display {
                    Some(Display::Block)
                    | Some(Display::GridLanes)
                    | Some(Display::ListItem)
                    | Some(Display::Flex)
                    | Some(Display::Grid) => true,
                    None => style.inline_display != Some(true) && BLOCK_TAGS.contains(&tag.as_str()),
                    _ => false,
                };
                block && !special && style.float.is_none_or(|f| f == 0)
            };
            // css-will-change-1: обещанный `transform`/`contain` делает коробку
            // содержащим блоком и контекстом наложения лишь там, где само
            // свойство применимо. У строчной НЕатомарной коробки его нет
            // (`will-change-transform-inline`: `fixed` внутри `<span>` стоит от
            // окна); замещаемые и вынесенные из потока — атомарны. Вид коробки
            // известен только здесь, после `finish_inline_display`.
            if style.will_change & crate::style::computed::wc::BOX != 0 {
                let out_of_flow = style.float.is_some_and(|f| f != 0)
                    || matches!(style.position, Some(Position::Absolute) | Some(Position::Fixed));
                let replaced = matches!(
                    tag.as_str(),
                    "img" | "svg" | "input" | "select" | "textarea" | "button" | "video"
                        | "canvas" | "iframe" | "object" | "embed" | "meter" | "progress"
                );
                let inline_tag =
                    INLINE_TAGS.contains(&tag.as_str()) || !BLOCK_TAGS.contains(&tag.as_str());
                let non_atomic = !out_of_flow
                    && !replaced
                    && (style.inline_display == Some(true)
                        || (style.display.is_none() && inline_tag));
                if !non_atomic {
                    use crate::style::computed::wc;
                    style.will_change |= wc::CB_ABS | wc::CB_FIXED | wc::STACK;
                }
            }
            // `transform-style` applies only to transformable elements
            // (css-transforms-2): a non-atomic inline with `preserve-3d` is no
            // 3D context, stacking context or containing block
            // (`preserve-3d-flat-grouping-properties-containing-block-inline`).
            if style.preserve_3d == Some(true) {
                let out_of_flow = style.float.is_some_and(|f| f != 0)
                    || matches!(style.position, Some(Position::Absolute) | Some(Position::Fixed));
                let inline_tag =
                    INLINE_TAGS.contains(&tag.as_str()) || !BLOCK_TAGS.contains(&tag.as_str());
                let replaced = matches!(
                    tag.as_str(),
                    "img" | "svg" | "input" | "select" | "textarea" | "button" | "video"
                        | "canvas" | "iframe" | "object" | "embed" | "meter" | "progress"
                );
                if !out_of_flow
                    && !replaced
                    && (style.inline_display == Some(true)
                        || (style.display.is_none() && inline_tag))
                {
                    style.preserve_3d = None;
                    style.frame_3d = None;
                }
            }
            inlinify_in_ruby(&mut style, &tag, path.iter().rev());
            // css-ruby-1 §3.3: «Neither the margin, padding, and border
            // properties … apply to base containers or annotation containers»
            // (`ruby-box-model-001`: `.rbc.pv { padding: 100px }` не должен
            // отодвигать аннотацию от базы). Контейнер — по тегу или по роли.
            if matches!(tag.as_str(), "rbc" | "rtc")
                || matches!(
                    style.ruby_role,
                    Some(crate::style::computed::RubyRole::BaseContainer)
                        | Some(crate::style::computed::RubyRole::TextContainer)
                )
            {
                style.margin = crate::style::computed::Sides::default();
                style.padding = crate::style::computed::Sides::default();
                style.border_width = crate::style::computed::Sides::default();
            }
            // motion-1: offset-трансформ считается НЕ здесь, а вторым проходом
            // по дереву коробок (`motion::settle`, зовётся из `doc.rs`).
            // §offset-path: «In CSS contexts, the boxes being referenced are
            // from the element that establishes the containing block for this
            // element» — опорная коробка `<coord-box>`, длина `ray()` и начало
            // `at <position>` берутся у СОДЕРЖАЩЕГО БЛОКА, а на разборе стиля
            // родителя нет вовсе: сюда доезжает только собственный каскад.
            // Язык — свойство узла, а не CSS: по нему выбираются образцы
            // слогораздела (`hyphens: auto`).
            if let Some((_, v)) = attrs.iter().find(|(k, _)| k == "lang") {
                style.lang = Some(v.clone());
            }
            apply_direction(&mut style, &tag, &attrs);
            // Правила с `:hover` собираются отдельным слоем: в базовый стиль
            // им нельзя, иначе элемент выглядел бы всегда наведённым.
            let mut hovered: Vec<&Rule> = rules
                .iter()
                .filter(|r| r.sel.pseudo.as_deref() == Some("hover"))
                .filter(|r| matches_ignoring_pseudo(&r.sel, &me, path, sibs))
                .collect();
            hovered.sort_by_key(|r| (r.sel.specificity(), r.order));
            // Слой наведения собирается ПОВЕРХ базового стиля, а не с нуля:
            // правило `:hover` меняет два-три свойства, а не весь стиль. Со
            // сборкой «с нуля» плавный переход на середине пути показывал
            // голый огрызок — без отступов, размеров и шрифта.
            let hover = (!hovered.is_empty()).then(|| {
                let mut merged = style.clone();
                for rule in hovered.iter() {
                    merged.apply_decls_with_vars(&rule.decls, vars);
                }
                merged
            });
            // Псевдоэлементы первой буквы и первой строки — тем же слоем
            // поверх базового стиля: они меняют начертание куска, а не блок.
            let layer = |name, base| initial_pseudos::resolve(name, rules, vars, &me, path, sibs, base);
            let first_letter = layer("first-letter", Some(&style));
            let first_line = layer("first-line", Some(&style));
            let first_line_own = layer("first-line", None).map(Box::new);
            style.first_letter_own = layer("first-letter", None).map(Box::new);
            style.first_line_own = first_line_own;
            // `::marker` — НЕ копией стиля хозяина, как первая буква, а
            // ТОЛЬКО своими объявлениями поверх таблицы агента: копия
            // протащила бы в маркер рамку, поля и размеры самого `<li>`.
            // CSS Lists 3 §3.1.1 gives marker text its own transform default,
            // including when no author ::marker rule matches.
            // Сворачивается ниже, ПОСЛЕ снятия номера пункта:
            // `counter(list-item)` в его `content` обязан видеть своё
            // значение.
            let marker_layer = {
                let mut found: Vec<&Rule> = rules
                    .iter()
                    .filter(|r| r.sel.pseudo.as_deref() == Some("marker"))
                    .filter(|r| matches_ignoring_pseudo(&r.sel, &me, path, sibs))
                    .collect();
                found.sort_by_key(|r| (r.sel.specificity(), r.order));
                (!found.is_empty() || tag == "li" || style.display == Some(Display::ListItem)).then(|| {
                    let mut m = Computed::default();
                    m.text_transform = Some(crate::style::computed::TextTransform::None);
                    if !found.is_empty() {
                        m.bidi_isolate = Some(true);
                    }
                    for rule in found.iter() {
                        m.apply_decls_with_vars(&rule.decls, vars);
                    }
                    m
                })
            };

            // Обратный счётчик без числа: начальное значение — итог
            // предварительного обхода области (css-lists-3
            // §instantiating-counters). Считается ЗДЕСЬ, до применения
            // директив: запись создаётся уже готовым числом.
            let reversed_start = |nm: &str, counters: &mut crate::style::generated::counters::Counters| {
                crate::style::generated::counters_scan::reversed_initial(
                    rules, vars, nm, handle, &me, path, sibs, level, spots, level_pos,
                )
            };

            if style.display == Some(Display::None) {
                // Table columns use an internal non-flow display, but still
                // generate boxes (CSS 2.1 §17.2). Their counter directives
                // apply; actual display:none nodes have no counters (§12.4.3).
                // CSS 2.1 §14.2: the root background paints the canvas even
                // without a root box. Keep its style for canvas propagation.
                if me.tag == "html" {
                    out.push(Node::Element(Element {
                        tag: "html".to_string(),
                        inline: false,
                        node_id: 0,
                        style,
                        hover: None,
                        first_letter: None,
                        first_line: None,
                        children: vec![],
                        attrs: vec![],
                        anim: Default::default(),
                        list_item: None,
                    }));
                    return;
                }
                let Some(role) = style.col_role else {
                    return;
                };
                counters.enter();
                apply_counter_decls(
                    &style, counters, &tag, &attrs, &mut false, &reversed_start,
                );
                // §17.2.1: у колонки детей нет вовсе, у группы колонок
                // остаются только колонки.
                let mut kids: Vec<Node> = vec![];
                if role == 1 {
                    let mut path2 = path.to_vec();
                    path2.push(me.clone());
                    let mut raw: Vec<Node> = vec![];
                    walk_children(
                        handle,
                        rules,
                        vars,
                        frames,
                        counter,
                        counters,
                        &path2,
                        style.preserve_newlines.unwrap_or(preserve),
                        &mut raw,
                    );
                    kids = raw
                        .into_iter()
                        .filter(|n| matches!(n, Node::Element(c) if c.style.col_role == Some(0)))
                        .collect();
                }
                counters.leave();
                *counter += 1;
                out.push(Node::Element(Element {
                    list_item: None,
                    node_id: *counter,
                    anim: None,
                    inline: false,
                    tag,
                    style,
                    hover: None,
                    first_letter: None,
                    first_line: None,
                    children: kids,
                    attrs,
                }));
                return;
            }

            // Counters apply before children; display:contents has no box level.
            let box_level = style.display != Some(Display::Contents);
            if box_level {
                counters.enter();
                counters.set_quote_language(language::parent(&me, path).unwrap_or(""));
                if let Some(q) = &style.quotes {
                    counters.set_quotes(q.clone());
                }
            }
            let mut is_list_item = false;
            apply_counter_decls(
                &style,
                counters,
                &tag,
                &attrs,
                &mut is_list_item,
                &reversed_start,
            );
            // Номер пункта снимается СРАЗУ после своих директив — до
            // псевдоэлементов и детей, которые счётчик двигают дальше.
            let list_item = is_list_item.then(|| counters.value_of("list-item"));
            let style_scope = (box_level && style.contain_style == Some(true))
                .then(|| counters.enter_style_scope());
            // Содержимое маркера — по первому верному условию css-lists-3
            // §content-property: `content` на `::marker` не `normal` →
            // «exactly as for ::before»; `none` → коробки нет; иначе
            // прежний путь `list-style-*`. Blink делает ту же отсечку
            // первой строкой `ListMarker::MarkerText`:
            // `if (!marker.StyleRef().ContentBehavesAsNormal()) return
            // kNotText;` (`list_marker.cc:159`).
            //
            // Своей коробки у маркера в дереве нет — его рисует
            // `render::list` по `marker_text`/`no_marker`, поэтому
            // содержимое сворачивается в эти поля, а прочие свойства слоя
            // (цвет, шрифт, разрядка) едут в `marker_layer`.
            //
            // CSS Lists 3 §marker-pseudo: the marker is the item's FIRST child,
            // before ::before; counter-* declared on ::marker (with DOM
            // inheritance of counter directives) apply right here, before the
            // marker's own content is evaluated (`marker-counter`).
            if let Some(mut m) = marker_layer {
                inherit_counter_decls(&mut m, Some(&me.counter_style));
                counters.enter_marker();
                language::pseudo(counters, &m, &me, path);
                apply_counter_decls(&m, counters, "", &[], &mut false, &|_, _| 0);
                if let Some(items) = host_content(&m, &me) {
                    let quotes = m.quotes.as_ref();
                    style.marker_text = Some(content_text(
                        &items,
                        counters,
                        &attrs,
                        quotes,
                        me.html_attrs,
                    ));
                    style.no_marker = Some(false);
                } else if m.content_none == Some(true) {
                    style.no_marker = Some(true);
                }
                counters.leave();
                style.marker_layer = Some(Box::new(m));
            }

            let mut path2 = path.to_vec();
            path2.push(me.clone());
            let mut children = vec![];
            // Псевдоэлементы: коробка появляется, только если у правила есть
            // `content`. Значками, стрелками и разделителями в вёрстке
            // занимаются именно они, и без них разметка теряет часть смысла.
            // `::before` строится ДО детей, `::after` — после: счётчики они
            // видят в том же порядке, что и браузер (css-lists §counters).
            if let Some(el) = pseudo_box(
                rules,
                vars,
                counters,
                &me,
                path,
                sibs,
                "before",
                &attrs,
            ) {
                children.push(Node::Element(el));
            }
            let keep = style.preserve_newlines.unwrap_or(preserve);
            if let Some(shadow) = &shadow {
                // Плоское дерево (css-shadow-1 §3.3): хост наполняется детьми
                // корня тени вместо светлых — в области стилей тени и с
                // цепочкой предков из одного безликого хоста. Светлые дети
                // попадут в вывод только через `<slot>`.
                walk_children(
                    &shadow.root,
                    &shadow.scope.rules,
                    vars,
                    &shadow.scope.frames,
                    counter,
                    counters,
                    std::slice::from_ref(&shadow.marker),
                    keep,
                    &mut children,
                );
            } else if let Some(slot) = slot_of(handle).filter(|s| !s.assigned.is_empty()) {
                // Слот показывает распределённые узлы, fallback — только без
                // них (HTML §4.12.4). Распределённый ребёнок стилизуется
                // таблицами ОБЛАСТИ ХОСТА и сопоставляется в светлом контексте
                // (предки хоста + хост, светлые братья), а наследует — от
                // слота, своего родителя в плоском дереве (`vars`, `keep`).
                for s in &slot.assigned {
                    let sibs = Sibs {
                        all: &slot.light_all[..],
                        pos: s.elem_pos,
                        is_elem: s.anc.is_some(),
                        rc: Some(&slot.light_all),
                    };
                    walk(
                        &s.node,
                        &slot.outer.rules,
                        vars,
                        &slot.outer.frames,
                        counter,
                        counters,
                        &slot.host_path[..],
                        slot.light_spots[s.light_idx],
                        keep,
                        sibs,
                        &slot.light,
                        &slot.light_spots,
                        s.light_idx,
                        &mut children,
                    );
                }
            } else {
                walk_children(
                    handle, rules, vars, frames, counter, counters, &path2, keep, &mut children,
                );
            }
            if let Some(el) = pseudo_box(
                rules,
                vars,
                counters,
                &me,
                path,
                sibs,
                "after",
                &attrs,
            ) {
                children.push(Node::Element(el));
            }
            // css-overflow-5: скроллер со `scroll-marker-group` собирает
            // `::scroll-marker` потомков в группу-соседа (у корня — ребёнка),
            // скроллер без группы их гасит; кнопки `::scroll-button()` — по
            // `content`. Свой маркер элемента кладётся ПОСЛЕ сбора: он идёт
            // в группу ВНЕШНЕГО скроллера, а не в собственную.
            let scroll_pseudos = scroll_marker_pass(
                rules,
                vars,
                counters,
                &me,
                path,
                sibs,
                &tag,
                &style,
                &attrs,
                &mut children,
            );
            if let Some(el) = pseudo_box(
                rules,
                vars,
                counters,
                &me,
                path,
                sibs,
                "scroll-marker",
                &attrs,
            ) {
                children.push(Node::Element(el));
            }
            // Колонка значит что-то ТОЛЬКО внутри таблицы или группы
            // колонок. У любого другого родителя она исчезает ровно так же,
            // как исчезала до сих пор: `empty-cells-applies-to-012` ставит
            // `display: table-column` с красным фоном ВНУТРИ ряда и требует
            // «no red». Без этой отсечки такой узел уехал бы в анонимную
            // ячейку и покрасился.
            let holds_columns = matches!(tag.as_str(), "table" | "colgroup")
                || style.col_role == Some(1)
                || matches!(
                    style.display,
                    Some(Display::Table) | Some(Display::InlineTable)
                );
            if !holds_columns {
                children.retain(|n| !matches!(n, Node::Element(c) if c.style.col_role.is_some()));
            }
            // CSS Lists §12.4.1: ordinary counters survive into following siblings.
            // Restore only the isolated subtree; remove_stale handles ordinary scopes.
            if let Some(scope) = style_scope {
                counters.leave_style_scope(scope);
            }
            if box_level {
                counters.leave();
            }

            *counter += 1;
            // Неявный якорь псевдоэлемента — порождающий элемент
            // (css-anchor-position-1 §implicit); его `node_id` известен
            // только здесь, после обхода детей: `::before` собран до них,
            // `::after` — после, а номер хозяину даёт этот же счётчик.
            for kid in children.iter_mut() {
                if let Node::Element(c) = kid
                    && c.tag.starts_with("::")
                {
                    c.style.implicit_anchor = Some(*counter);
                }
            }
            // Кадры разрешаются здесь же: к моменту отрисовки таблицы стилей
            // уже нет, а интерполировать нужно готовые стили, а не текст.
            let anim = style.animation.as_ref().and_then(|a| {
                // Набор `name` поверх стиля `base`.
                let resolve = |name: &str, base: &Computed| -> Option<Vec<(f32, Computed)>> {
                    let track = frames.get(name)?;
                    let mut resolved: Vec<(f32, Computed)> = track
                        .iter()
                        .map(|(at, decls)| {
                            let mut c = base.clone();
                            // Кадр ЗАМЕНЯЕТ `transform`, а разбор свойства
                            // дописывает функции к уже стоящим (ветка
                            // `"transform"`: `self.transform.unwrap_or_default()`).
                            if decls.contains_key("transform") {
                                c.transform = None;
                            }
                            c.apply_decls_with_vars(decls, vars);
                            (*at, c)
                        })
                        .collect();
                    // Недостающие `0%`/`100%` строятся из вычисленного стиля
                    // (css-animations-1 §keyframes). Только у остановленной:
                    // живая обёртка с таким кадром сделала бы reftest
                    // недетерминированным (`individual-transform-combine`: пять
                    // наборов из одного `to`).
                    if a.frozen() {
                        if resolved.first().is_some_and(|f| f.0 > 0.0) {
                            resolved.insert(0, (0.0, base.clone()));
                        }
                        if resolved.last().is_some_and(|f| f.0 < 1.0) {
                            resolved.push((1.0, base.clone()));
                        }
                    }
                    (resolved.len() >= 2).then_some(resolved)
                };
                // Несколько остановленных анимаций — слоями по порядку списка
                // (css-animations-1 §3: при общем свойстве побеждает имя,
                // стоящее позже); итог — постоянный набор из двух одинаковых
                // кадров (`individual-transform-ordering`: `anim-7, anim-8`).
                if a.frozen() && a.names.len() > 1 {
                    let t = a.frozen_t();
                    let mut base = style.clone();
                    for name in &a.names {
                        if let Some(track) = resolve(name, &base) {
                            base = crate::animation::frames::frame_at(&track, t);
                        }
                    }
                    return Some(vec![(0.0, base.clone()), (1.0, base)]);
                }
                resolve(&a.name, &style)
            });
            // `dir="auto"` — сторону задаёт ПЕРВЫЙ СИЛЬНЫЙ знак содержимого
            // (HTML §3.2.6.4). Раньше здесь не ставилось ничего в расчёте на
            // разбор двунаправленности, но он берёт сторону абзаца, а не
            // куска: строка `1;234;56א;` внутри `<span dir=auto>` выходила
            // слева направо (`empty-span-001`).
            if attrs
                .iter()
                .any(|(k, v)| k == "dir" && v.eq_ignore_ascii_case("auto"))
            {
                // `dir="auto"` в HTML — это `unicode-bidi: plaintext`: сторона
                // решается для КАЖДОГО абзаца между жёсткими разрывами, а не
                // для элемента целиком (`text-align-end-016`).
                style.bidi_plaintext = Some(true);
                if style.rtl.is_none()
                    && let Some(rtl) = first_strong(&children)
                {
                    style.rtl = Some(rtl);
                }
            }
            // Псевдокоробки ВНЕ элемента (css-overflow-5; порядок Blink
            // `kBoxTreeOrder`): группа `before` — перед ним, кнопки
            // (block-start, inline-start, inline-end, block-end) и группа
            // `after` — за ним. У корня всё это — его первый/последние дети
            // (§scroll-marker-group: «first child of the originating
            // element»). Неявный якорь у них — сам элемент.
            let mut sp = scroll_pseudos;
            for el in sp
                .group_before
                .iter_mut()
                .chain(sp.buttons.iter_mut())
                .chain(sp.group_after.iter_mut())
            {
                el.style.implicit_anchor = Some(*counter);
            }
            if tag == "html" {
                if let Some(g) = sp.group_before.take() {
                    children.insert(0, Node::Element(g));
                }
                children.extend(sp.buttons.drain(..).map(Node::Element));
                if let Some(g) = sp.group_after.take() {
                    children.push(Node::Element(g));
                }
            } else if let Some(g) = sp.group_before.take() {
                out.push(Node::Element(g));
            }
            // css-ruby-1 §2.1.2 «Non-Inline Ruby»: `display: block ruby` даёт
            // ДВЕ коробки — главную блочную и строчный контейнер руби внутри
            // (Blink `LayoutRubyAsBlock::AddChild`: первый ребёнок — анонимный
            // `LayoutInline` с `display: ruby`, все дети идут в него). Свойства
            // элемента — на главной коробке; наследуемые доходят до контейнера
            // обычным `inline::inherit` (стиль контейнера пуст). Тег `ruby` у
            // синтетического узла — роль контейнера по тегу (`block-ruby-001`).
            let children = if style.display == Some(Display::Block)
                && style.ruby_role == Some(crate::style::computed::RubyRole::Container)
            {
                vec![Node::Element(Element {
                    list_item: None,
                    node_id: 0,
                    anim: None,
                    inline: true,
                    tag: "ruby".to_string(),
                    style: Computed::default(),
                    hover: None,
                    first_letter: None,
                    first_line: None,
                    children,
                    attrs: vec![],
                })]
            } else {
                children
            };
            let children = if ruby_box_role(&tag, &style).is_none() {
                wrap_misparented_ruby(children)
            } else {
                children
            };
            let inline = INLINE_TAGS.contains(&tag.as_str()) || !BLOCK_TAGS.contains(&tag.as_str());
            style.block_tag = !inline;
            // Замена элемента (css-content-3 §content-property: «a single
            // <image>» на самом элементе): коробка становится замещаемой
            // картинкой, содержимое не рисуется. Уровень коробки остаётся от
            // исходного тега и `display` — `<p>` замещается блоком. Корень не
            // трогается: замещаемого корня у нас нет. Ненайденная картинка
            // замены не делает (Servo `replaced.rs:348`: `None` при ошибке).
            let (tag, children, attrs) = match style.content.as_deref() {
                Some([crate::style::computed::ContentItem::Image(src)])
                    if tag != "html" && content_image_src(src).is_some() =>
                {
                    let mut attrs: Vec<(String, String)> =
                        attrs.into_iter().filter(|(k, _)| k != "src" && k != "srcset").collect();
                    attrs.push(("src".into(), content_image_src(src).unwrap_or_default()));
                    ("img".to_string(), vec![], attrs)
                }
                _ => (tag, children, attrs),
            };
            out.push(Node::Element(Element {
                list_item,
                node_id: *counter,
                anim,
                inline,
                tag,
                style,
                hover,
                first_letter,
                first_line,
                children,
                attrs,
            }));
            out.extend(sp.buttons.into_iter().map(Node::Element));
            if let Some(g) = sp.group_after {
                out.push(Node::Element(g));
            }
        }
        _ => walk_children(
            handle, rules, vars, frames, counter, counters, path, preserve, out,
        ),
    }
}
