//! Регистрация at-правил и выбор условных групп таблицы стилей.

use super::*;

/// Разобрать at-правило `name` из `sheet_rules`: правила из тела применяются
/// (`true`) у подошедших `@media`/`@supports` и у `@layer`/`@scope`; `@font-feature-values`,
/// `@page`, `@property`, `@counter-style`, `@position-try` обрабатываются здесь же и дают `false`.
pub(super) fn at_rule_inner(
    name: &str,
    head: &str,
    body: &str,
    media: &Media,
    cx: &SheetCx,
) -> bool {
    let vcx = cx.doc.values();
    let layer_of_rules = || cx.layer_of_rules();
    if name.starts_with("@media") {
        media.matches_in(name, cx.custom(), vcx)
    } else if name
        .strip_prefix("@font-feature-values")
        .is_some_and(|s| s.starts_with(char::is_whitespace))
    {
        crate::style::computed::font_alternates::register(&head[20..], body, layer_of_rules(), vcx);
        false
    } else if name.starts_with("@page") {
        // Правило с головой (имя, `:first/:left/:right/:blank`,
        // список) — в реестр документа, каскад по листу решает
        // `page_decls_in`. Имя регистрозависимо (css-page-3
        // §using-named-pages) — из ОРИГИНАЛА головы, не из `name`.
        let flat = strip_nested_blocks(body);
        let decls = parse_decls_in(&flat, vcx);
        // Марджин-боксы — вложенные at-правила с известным именем.
        let margins: Vec<(String, DeclBlock)> = nested_blocks(body)
            .into_iter()
            .filter(|(n, _)| MARGIN_BOXES.contains(&n.as_str()))
            .map(|(n, b)| (n, parse_decls_in(&b, vcx)))
            .collect();
        if !decls.is_empty() || !margins.is_empty() {
            // Объявления листа — В ПОРЯДКЕ ЗАПИСИ, повтор свойства — на
            // своём месте, и `page_box` применяет их по очереди:
            // `margin: 0; margin-top: 20vw` (`page-size-016`),
            // `margin: 13px; margin: inherit` (`page-margin-006`).
            if let Some(sels) = parse_page_selectors(&head[5..]) {
                push_page_rule(
                    cx.doc,
                    PageRule {
                        sels,
                        decls,
                        margins,
                        layer: layer_of_rules(),
                    },
                );
            }
        }
        false
    } else if name.starts_with("@property") {
        // css-properties-values-api-1 §3: правило действительно, только
        // если заданы `syntax` и `inherits`, а при синтаксисе не `*` —
        // ещё и `initial-value`. Имя — с исходным регистром.
        let ident = head["@property".len()..].trim();
        let decls = parse_decls_in(body, cx.doc.values());
        let syntax = decls.descriptor("syntax").map(|s| {
            s.trim()
                .trim_matches(|c| c == '"' || c == '\'')
                .trim()
                .to_string()
        });
        let inherits = decls
            .descriptor("inherits")
            .map(|s| s.trim().to_ascii_lowercase());
        let initial = decls
            .descriptor("initial-value")
            .map(|s| s.trim().to_string());
        if ident.starts_with("--")
            && let (Some(syntax), Some(inherits)) = (syntax, inherits)
            && matches!(inherits.as_str(), "true" | "false")
            && (syntax == "*" || initial.is_some())
        {
            register_property(
                cx.doc,
                ident,
                Registered {
                    syntax,
                    inherits: inherits == "true",
                    initial,
                },
            );
        }
        false
    } else if name
        .strip_prefix("@counter-style")
        .is_some_and(|s| s.starts_with(char::is_whitespace))
    {
        // css-counter-styles-3 §3: дескрипторы правила — в реестр
        // документа, повторы — по порядку записи (последнее
        // действительное побеждает). Имя — с исходным регистром.
        let decls = parse_decls_in(body, cx.doc.values());
        crate::style::generated::counter_style_rules::register(
            cx.doc,
            &head["@counter-style".len()..],
            &decls,
        );
        false
    } else if name.starts_with("@position-try") {
        // §fallback-rule: тело — обычные объявления (только вставки,
        // поля, размеры, самовыравнивание, `position-anchor`,
        // `position-area`; лишние здесь безвредны — накладываются на
        // копию стиля кандидата). Имя — с оригинальным регистром.
        let ident = head["@position-try".len()..].trim();
        if ident.starts_with("--") {
            let decls = parse_decls_in(body, cx.doc.values());
            if !decls.is_empty() {
                insert_try_rule(cx.doc, ident, decls);
            }
        }
        false
    } else if name.starts_with("@supports") {
        // Условию нужен ОРИГИНАЛ: лоуеркейс головы ломал значения
        // (`(font-family: "Foo")`); имя правила — ASCII, срез безопасен.
        supports_in(&head[9..], &cx.selectors, vcx)
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
