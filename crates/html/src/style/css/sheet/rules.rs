//! Обход правил таблицы стилей (sheet_rules): @-правила, вложенность, слои, @import/@media/@supports/@scope, сбор Rule.

use super::*;

pub(super) fn sheet_rules(css: &str, media: Media, top: bool) -> Vec<Rule> {
    let mut out = vec![];
    let cleaned = strip_comments(css);
    let mut rest = cleaned.as_str();
    let mut order = 0usize;
    loop {
        if top {
            rest = stylesheet_tokens::start(rest);
        }
        let Some((piece, tail)) = next_piece(rest) else {
            break;
        };
        rest = tail;
        // At-правило-ПРЕДЛОЖЕНИЕ блока не имеет и кончается точкой с запятой:
        // `@import`, `@charset`, `@namespace`, `@layer a, b;`. Ни одно из них
        // ничего не задаёт нашей отрисовке, поэтому запись просто пропускается
        // вместе со всей своей преамбулой.
        let (head, body) = match piece {
            // `@layer a, b.c;` — объявление порядка слоёв без правил
            // (css-cascade-5 §6.4.2): имена регистрируются по месту.
            Piece::Statement { head } => {
                let h = head.trim();
                // `get`, а не срез: шестой байт бывает внутри многобайтового
                // знака (`@chars…` в чужой кодировке, `at-charset-029`), и
                // срез ронял весь стенд паникой.
                if h.len() > 6 && h.get(..6).is_some_and(|p| p.eq_ignore_ascii_case("@layer")) {
                    for name in h[6..].split(',') {
                        let name = name.trim();
                        if !name.is_empty() {
                            let _ = layer_enter_path(name);
                        }
                    }
                }
                continue;
            }
            Piece::Block { head, body } => (head.trim(), body),
        };
        // Незакрытый блок в КОНЦЕ таблицы закрывается неявно (CSS Syntax
        // §5.4.1): правило всё равно действует. Прежде такое правило
        // отбрасывалось целиком — а в наборе оно встречается прямо в тесте
        // (`break-spaces-009`: у `.test` нет закрывающей скобки, и коробка
        // теряла свою ширину вместе со всем остальным).
        // At-правила: тело у них устроено иначе, поэтому обычными правилами
        // их применять нельзя. `@media` и `@supports` разбираются как обёртка
        // над обычными правилами, `@keyframes` — отдельно (см.
        // `parse_keyframes`), остальные пропускаются.
        if head.starts_with('@') {
            // Имя at-правила регистронезависимо (§3.3): `@MeDIa` — то же
            // самое, что `@media` (`case-sensitive-001`).
            let name = head.to_ascii_lowercase();
            let inner = at_rule_inner(&name, head, body, &media);
            // Блок `@layer имя { … }`: правила внутри — в своём слое.
            let layer_block = name.starts_with("@layer");
            let saved_layer = layer_block.then(|| {
                let entered = layer_enter_path(head[6..].trim());
                LAYER_NOW.with(|l| std::mem::replace(&mut *l.borrow_mut(), entered))
            });
            let inner_rules = if inner {
                parse_stylesheet_media(body, media)
            } else {
                vec![]
            };
            if let Some(saved) = saved_layer {
                LAYER_NOW.with(|l| *l.borrow_mut() = saved);
            }
            if inner {
                for r in inner_rules {
                    out.push(Rule {
                        order: order + r.order,
                        ..r
                    });
                }
                order += 1000;
            }
            continue;
        }
        let decls = parse_decls(body);
        if decls.is_empty() {
            continue;
        }
        // Список селекторов режется ВНЕ скобок: запятая внутри
        // `:nth-child(2n of #a, #b)` — часть of-списка, а не граница
        // селектора, иначе `#b` становился самостоятельным правилом.
        let parts = split_top_level(head, ',');
        // Пустая часть списка селекторов делает недействительным ВЕСЬ
        // список (CSS 2.1 §4.1.7): `body,,div {}` не применяется ни к кому.
        if parts.iter().any(|one| one.trim().is_empty()) {
            continue;
        }
        // Недействительная часть списка тоже валит ВЕСЬ список (§4.1.7):
        // `p:first-line.two, p.two {}` не применяется ни к кому. Прежде
        // негодная часть просто выбрасывалась, и правило красило соседа
        // (`c25-pseudo-elmnt-000`).
        let sels: Option<Vec<Selector>> = parts.iter().map(|one| Selector::parse(one)).collect();
        let Some(sels) = sels else { continue };
        for sel in sels {
            {
                out.push(Rule {
                    sel,
                    decls: decls.clone(),
                    order,
                    // Разбор не знает, чья это таблица: происхождение ставит
                    // тот, кто её подключает (см. `dom.rs`).
                    origin: 0,
                    layer: layer_of_rules(),
                });
                order += 1;
            }
        }
    }
    out
}

/// Разобрать at-правило `name` из `sheet_rules`: правила из тела применяются
/// (`true`) у подошедших `@media`/`@supports` и у `@layer`/`@scope`; `@font-feature-values`,
/// `@page`, `@property`, `@counter-style`, `@position-try` обрабатываются здесь же и дают `false`.
fn at_rule_inner(name: &str, head: &str, body: &str, media: &Media) -> bool {
    if name.starts_with("@media") {
        media.matches(&name)
    } else if name
        .strip_prefix("@font-feature-values")
        .is_some_and(|s| s.starts_with(char::is_whitespace))
    {
        crate::text::fonts::alternates::register(&head[20..], body, layer_of_rules());
        false
    } else if name.starts_with("@page") {
        // Правило с головой (имя, `:first/:left/:right/:blank`,
        // список) — в пул `PAGE_RULES`, каскад по листу решает
        // `page_decls_in`. Имя регистрозависимо (css-page-3
        // §using-named-pages) — из ОРИГИНАЛА головы, не из `name`.
        let flat = strip_nested_blocks(body);
        let decls = parse_decls(&flat);
        // Марджин-боксы — вложенные at-правила с известным именем.
        let margins: Vec<(String, Vec<(String, String)>)> = nested_blocks(body)
            .into_iter()
            .filter(|(n, _)| MARGIN_BOXES.contains(&n.as_str()))
            .map(|(n, b)| (n, ordered_decls(&parse_decls(&b))))
            .collect();
        if !decls.is_empty() || !margins.is_empty() {
            // Объявления листа — В ПОРЯДКЕ ЗАПИСИ, повтор свойства —
            // отдельной парой на своём месте. Словарь `Decls` порядка не
            // помнит (случайный `RandomState` на процесс), повтор
            // склеивает через `DECL_SEP`, а `page_box` стенда применяет
            // пары по очереди: `margin: 0; margin-top: 20vw`
            // (`page-size-016`) давал разный лист от прогона к прогону,
            // а `margin: 13px; margin: inherit` (`page-margin-006`) —
            // значение «13px\u{1}inherit», которое не разбиралось вовсе.
            // Служебный `ORDER_KEY` в пул больше не попадает.
            let list = ordered_decls(&decls);
            if let Some(sels) = parse_page_selectors(&head[5..]) {
                PAGE_RULES.lock().unwrap().push(PageRule {
                    sels,
                    decls: list,
                    margins,
                });
            }
        }
        false
    } else if name.starts_with("@property") {
        // css-properties-values-api-1 §3: правило действительно, только
        // если заданы `syntax` и `inherits`, а при синтаксисе не `*` —
        // ещё и `initial-value`. Имя — с исходным регистром.
        let ident = head["@property".len()..].trim();
        let decls = parse_decls(body);
        let syntax = decls.get("syntax").map(|s| {
            s.trim()
                .trim_matches(|c| c == '"' || c == '\'')
                .trim()
                .to_string()
        });
        let inherits = decls.get("inherits").map(|s| s.trim().to_ascii_lowercase());
        let initial = decls.get("initial-value").map(|s| s.trim().to_string());
        if ident.starts_with("--")
            && let (Some(syntax), Some(inherits)) = (syntax, inherits)
            && matches!(inherits.as_str(), "true" | "false")
            && (syntax == "*" || initial.is_some())
        {
            PROPERTY_RULES
                .lock()
                .unwrap()
                .get_or_insert_with(HashMap::new)
                .entry(ident.to_string())
                .or_insert(Registered {
                    syntax,
                    inherits: inherits == "true",
                    initial,
                });
        }
        false
    } else if name
        .strip_prefix("@counter-style")
        .is_some_and(|s| s.starts_with(char::is_whitespace))
    {
        // css-counter-styles-3 §3: дескрипторы правила — в реестр
        // документа, повторы — по порядку записи (последнее
        // действительное побеждает). Имя — с исходным регистром.
        let decls = parse_decls(body);
        let descs: Vec<(String, Vec<String>)> = decls
            .iter()
            .filter(|(k, _)| k.as_str() != ORDER_KEY)
            .map(|(k, v)| (k.clone(), v.split(DECL_SEP).map(str::to_string).collect()))
            .collect();
        crate::style::generated::counter_style_rules::register(
            &head["@counter-style".len()..],
            &descs,
        );
        false
    } else if name.starts_with("@position-try") {
        // §fallback-rule: тело — обычные объявления (только вставки,
        // поля, размеры, самовыравнивание, `position-anchor`,
        // `position-area`; лишние здесь безвредны — накладываются на
        // копию стиля кандидата). Имя — с оригинальным регистром.
        let ident = head["@position-try".len()..].trim();
        if ident.starts_with("--") {
            let decls = parse_decls(body);
            if !decls.is_empty() {
                TRY_RULES
                    .lock()
                    .unwrap()
                    .get_or_insert_with(HashMap::new)
                    .insert(ident.to_string(), decls);
            }
        }
        false
    } else if name.starts_with("@supports") {
        // Условию нужен ОРИГИНАЛ: лоуеркейс головы ломал значения
        // (`(font-family: "Foo")`); имя правила — ASCII, срез безопасен.
        supports(&head[9..])
    } else {
        // Слой — прозрачная обёртка: внутри обычные правила, и вся
        // разница в приоритете, которого у нас пока нет. Отбрасывая
        // блок целиком, мы теряли всю разметку современных наборов
        // стилей — они целиком лежат в `@layer`.
        //
        // `@scope` БЕЗ прелюдии — тоже прозрачная обёртка: корнем
        // области служит РОДИТЕЛЬ владельца таблицы (css-cascade-6
        // §3.1 — «If the <scope-start> is omitted, the scoping root is
        // the parent element of the owner node»), предела нет, и
        // селекторы внутри остаются обычными. Форма с прелюдией
        // (`@scope (.a) to (.b)`) по-прежнему выбрасывается: ей нужен
        // перенос корня области в сам селектор, иначе правило
        // расползётся за свою область.
        name.starts_with("@layer") || name == "@scope"
    }
}
