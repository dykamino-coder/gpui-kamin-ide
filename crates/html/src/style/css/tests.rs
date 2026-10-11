//! Тесты разбора таблиц стилей (style::css): объявления, селекторы, @-правила, медиа.

use super::*;

#[test]
fn decls_keep_commas_inside_functions() {
    let d = parse_decls("color: rgba(1, 2, 3, .5); padding : 4px ");
    assert_eq!(
        d.get("color").map(String::as_str),
        Some("rgba(1, 2, 3, .5)")
    );
    assert_eq!(d.get("padding").map(String::as_str), Some("4px"));
}

#[test]
fn layer_block_is_transparent_and_supports_not_is_honoured() {
    let media = Media::default();
    // Слой — обёртка: правило внутри него живёт.
    let rules = parse_stylesheet_media("@layer base { p { color: red } }", media);
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].decls.get("color").map(String::as_str), Some("red"));
    // Отрицание — запасная ветка для движка БЕЗ поддержки; применять её
    // нельзя, иначе применяются обе ветки пары сразу.
    let neg = parse_stylesheet_media("@supports not (display: grid) { p { color: red } }", media);
    assert!(neg.is_empty());
    let pos = parse_stylesheet_media("@supports (display: grid) { p { color: red } }", media);
    assert_eq!(pos.len(), 1);
}

#[test]
fn important_stays_in_the_value_for_the_cascade() {
    // Раньше тест закреплял обратное — что пометка срезается при разборе.
    // Именно из-за этого важность не работала вовсе: до каскада значение
    // доезжало неотличимым от обычного. Снимает пометку тот, кто
    // раскладывает каскад (`Computed::resolve_with_vars`).
    let d = parse_decls("color: red !important");
    assert_eq!(d.get("color").map(String::as_str), Some("red !important"));
}

#[test]
fn bang_that_is_not_important_kills_the_declaration() {
    // После восклицательного знака стоит ровно `important` (CSS 2.1
    // §4.1.8); всё прочее делает объявление недействительным целиком.
    let d = parse_decls("color: red ! fail; background: green");
    assert_eq!(d.get("color"), None);
    assert_eq!(d.get("background").map(String::as_str), Some("green"));
}

#[test]
fn selector_parts_and_specificity() {
    let s = Selector::parse("div.card#main:hover").unwrap();
    assert_eq!(s.tag.as_deref(), Some("div"));
    assert_eq!(s.id.as_deref(), Some("main"));
    assert_eq!(s.classes, vec!["card".to_string()]);
    assert_eq!(s.pseudo.as_deref(), Some("hover"));
    assert_eq!(s.specificity(), (1, 2, 1));
}

#[test]
fn descendant_and_child_combinators() {
    let s = Selector::parse(".card > .title").unwrap();
    assert_eq!(s.classes, vec!["title".to_string()]);
    let anc = s.ancestor.as_ref().unwrap();
    assert_eq!(anc.0.classes, vec!["card".to_string()]);
    assert!(anc.1, "после > предок обязан быть прямым");

    let s = Selector::parse(".card .title").unwrap();
    assert!(!s.ancestor.as_ref().unwrap().1, "пробел = любой предок");
}

#[test]
fn unsupported_selectors_are_dropped_whole() {
    // Атрибутные селекторы теперь разбираются.
    let attr = Selector::parse("a[href]").expect("наличие атрибута");
    assert_eq!(attr.attrs.len(), 1);
    assert!(attr.attrs[0].matches(Some("x")));
    assert!(!attr.attrs[0].matches(None));
    let eq = Selector::parse("input[type=\"text\" i]").expect("значение");
    assert!(eq.attrs[0].ci);
    assert!(eq.attrs[0].matches(Some("TEXT")));
    assert!(!eq.attrs[0].matches(Some("password")));
    let lang = Selector::parse("[lang|=en]").expect("дефисное");
    assert!(lang.attrs[0].matches(Some("en-US")));
    assert!(!lang.attrs[0].matches(Some("ent")));
    // Псевдоэлемент разбирается: коробку из него строит `dom.rs`.
    assert_eq!(
        Selector::parse("li::before").and_then(|s| s.pseudo),
        Some("before".to_string())
    );
    // Соседние комбинаторы разбираются: `+` — смежный, `~` — любой раньше.
    let adj = Selector::parse("h1 + p").expect("смежный сосед");
    assert_eq!(adj.tag.as_deref(), Some("p"));
    let prev = adj.prev.expect("сосед");
    assert_eq!(prev.0.tag.as_deref(), Some("h1"));
    assert!(prev.1, "`+` — смежный");
    let anywhere = Selector::parse("img ~ img").expect("общий сосед");
    assert!(!anywhere.prev.expect("сосед").1, "`~` — любой раньше");
    // Смешанная цепочка: предмет `.c`, его сосед `.b`, предок соседа `.a`.
    let mixed = Selector::parse(".a > .b + .c").expect("цепочка");
    assert_eq!(mixed.classes, vec!["c".to_string()]);
    let b = mixed.prev.expect("сосед");
    assert_eq!(b.0.classes, vec!["b".to_string()]);
    assert_eq!(
        b.0.ancestor.as_ref().expect("предок").0.classes,
        vec!["a".to_string()]
    );
}

#[test]
fn media_rules_apply_when_the_condition_holds() {
    let css = "
        /* заметка */
        .a { color: red }
        @media (min-width: 10px) { .b { color: blue } }
        .c, .d { padding: 2px }
    ";
    let rules = parse_stylesheet(css);
    let sels: Vec<String> = rules.iter().map(|r| r.sel.classes.join(",")).collect();
    // Условие выполнено при ширине по умолчанию — правило внутри работает.
    assert_eq!(sels, vec!["a", "b", "c", "d"]);
    assert_eq!(rules[0].decls.get("color").map(String::as_str), Some("red"));
}

#[test]
fn media_rules_are_skipped_when_the_condition_fails() {
    let css = "@media (min-width: 2000px) { .b { color: blue } }";
    let rules = parse_stylesheet_media(
        css,
        Media {
            width: 400.0,
            ..Media::default()
        },
    );
    assert!(rules.is_empty(), "узкое окно не берёт правило для широкого");
}

#[test]
fn color_scheme_query_follows_the_theme() {
    let css = "@media (prefers-color-scheme: dark) { .b { color: #fff } }";
    let dark = parse_stylesheet_media(
        css,
        Media {
            dark: true,
            ..Media::default()
        },
    );
    let light = parse_stylesheet_media(
        css,
        Media {
            dark: false,
            ..Media::default()
        },
    );
    assert_eq!(dark.len(), 1);
    assert!(light.is_empty());
}
