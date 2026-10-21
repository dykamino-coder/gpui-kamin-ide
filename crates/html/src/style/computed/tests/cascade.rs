//! Тесты каскада: var() с запасным значением, !important, приоритет авторской таблицы, сокращение
//! font.

#[test]
fn var_fallback_with_nested_parens_survives_a_defined_variable() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    // Конец записи — парная скобка, а запятая ищется на верхнем уровне:
    // иначе при ЗАДАННОЙ переменной оставалась лишняя скобка и значение
    // умирало, а при незаданной выходило случайно верно — из-за чего
    // дефект и не был виден.
    let mut vars = crate::style::css::VarMap::new();
    vars.insert("--c".into(), "red".into());
    let mut c = crate::style::computed::Computed::default();
    c.apply_decls_with_vars(
        &crate::style::css::parse_decls("color: var(--c, rgba(0,0,0,.5))"),
        &vars,
        vcx,
    );
    assert_eq!(
        c.color,
        crate::style::values::value::Color::parse("red", vcx)
    );
    // Незаданная переменная берёт запасное значение ЦЕЛИКОМ.
    let mut c = crate::style::computed::Computed::default();
    c.apply_decls_with_vars(
        &crate::style::css::parse_decls("color: var(--none, rgba(0,0,0,1))"),
        &crate::style::css::VarMap::new(),
        vcx,
    );
    assert_eq!(
        c.color,
        crate::style::values::value::Color::parse("rgba(0,0,0,1)", vcx)
    );
}

#[test]
fn important_survives_a_later_ordinary_rule() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    // Важность — самый старший ключ сравнения (CSS Cascade §6.1): важное
    // объявление раннего правила переживает обычное объявление позднего,
    // даже если то и специфичнее. Пока проходы шли внутри правила,
    // `!important` действовал только против соседей по своему блоку.
    let early = crate::style::css::Rule {
        sel: crate::style::css::Selector::parse("p").expect("селектор тега"),
        decls: crate::style::css::parse_decls("color: red !important"),
        order: 0,
        origin: 1,
        layer: vec![u32::MAX],
    };
    let late = crate::style::css::Rule {
        sel: crate::style::css::Selector::parse("p.x").expect("селектор класса"),
        decls: crate::style::css::parse_decls("color: green"),
        order: 1,
        origin: 1,
        layer: vec![u32::MAX],
    };
    let mut matched = vec![&early, &late];
    let c = crate::style::computed::Computed::resolve(
        &mut matched,
        &crate::style::css::DeclBlock::new(),
        vcx,
    );
    assert_eq!(
        c.color,
        crate::style::values::value::Color::parse("red", vcx)
    );
}

#[test]
fn author_sheet_beats_user_agent_regardless_of_specificity() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    // Происхождение старше специфичности (CSS Cascade §6.4.4). Пока обе
    // таблицы сравнивались только специфичностью, `* { margin: 0 }` со
    // специфичностью (0,0,0) проигрывал умолчанию `p { margin: 6px 0 }`
    // — то есть не работал ни один reset.
    let ua = crate::style::css::Rule {
        sel: crate::style::css::Selector::parse("p").expect("селектор тега"),
        decls: crate::style::css::parse_decls("margin-top: 6px"),
        order: 0,
        origin: 0,
        layer: vec![u32::MAX],
    };
    let author = crate::style::css::Rule {
        sel: crate::style::css::Selector::parse("*").expect("универсальный селектор"),
        decls: crate::style::css::parse_decls("margin-top: 0"),
        order: 1,
        origin: 1,
        layer: vec![u32::MAX],
    };
    let mut matched = vec![&ua, &author];
    let c = crate::style::computed::Computed::resolve(
        &mut matched,
        &crate::style::css::DeclBlock::new(),
        vcx,
    );
    assert_eq!(
        c.margin.top,
        Some(crate::style::values::value::Len::Px(0.0))
    );
}

#[test]
fn font_shorthand_takes_size_with_line_height_and_family() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    // Пробелы вокруг косой черты допустимы, и семейство начинается ПОСЛЕ
    // высоты строки: раньше «/ 1 Ahem» уезжало в семейство целиком, и
    // страница набиралась чужим шрифтом.
    let mut c = crate::style::computed::Computed::default();
    c.apply_one("font", "50px / 1 Ahem", vcx);
    assert_eq!(
        c.font_size,
        Some(crate::style::values::value::Len::Px(50.0))
    );
    assert_eq!(
        c.line_height,
        Some(crate::style::values::value::Len::Pct(1.0))
    );
    assert_eq!(c.font_family.as_deref(), Some("Ahem"));
}

/// `apply_decls` — тот же проход, что и каскад правил: исход не зависит от
/// обхода хеш-таблицы (каждый новый `HashMap` получает свой `RandomState`).
#[test]
fn apply_decls_is_deterministic_and_takes_the_cascade_order() {
    use crate::style::values::value::Len;
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..64 {
        let mut c = crate::style::computed::Computed::default();
        c.apply_decls(
            &crate::style::css::parse_decls("inset: 1px; top: 5px"),
            doc.values(),
        );
        seen.insert(format!("{:?}", c.inset.top));
    }
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert!(
        seen.contains(&format!("{:?}", Some(Len::Px(5.0)))),
        "{seen:?}"
    );
}

/// `!important` внутри блока побеждает обычное объявление того же свойства
/// и в `apply_decls` (второй проход), а не по случайному порядку частей.
#[test]
fn apply_decls_applies_important_after_normal() {
    use crate::style::values::value::Len;
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let mut c = crate::style::computed::Computed::default();
    c.apply_decls(
        &crate::style::css::parse_decls("top: 3px !important; top: 1px"),
        doc.values(),
    );
    assert_eq!(c.inset.top, Some(Len::Px(3.0)));
}
