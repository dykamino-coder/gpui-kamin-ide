//! Тесты порождения значений `@counter-style`: арифметика систем (`cyclic`,
//! `fixed`, …) на всей ширине `i64` и порядок дескрипторов, без раскладки.

use super::*;
use crate::document::state::ValueCx;

fn initial(algo: &Algo, v: i128) -> Option<String> {
    super::initial(algo, v, Context::default())
}
fn generate(
    rules: &HashMap<String, Raw>,
    v: i64,
    name: &str,
    depth: usize,
    vcx: ValueCx<'_>,
) -> String {
    super::generate(rules, v, name, depth, Context::default(), vcx)
}

#[test]
fn systems_cover_negative_fixed_and_full_width_values() {
    let syms = vec!["a".into(), "b".into()];
    let algo = |sys| Algo::Own(sys, syms.clone(), vec![]);
    assert_eq!(
        initial(&algo(System::Cyclic), i64::MIN.into()).as_deref(),
        Some("b")
    );
    assert_eq!(
        initial(&algo(System::Cyclic), i64::MAX.into()).as_deref(),
        Some("a")
    );
    assert_eq!(
        initial(&algo(System::Fixed(i64::MIN)), i64::MIN.into()).as_deref(),
        Some("a")
    );
    assert_eq!(
        initial(&algo(System::Fixed(i64::MIN)), i128::from(i64::MIN) + 1).as_deref(),
        Some("b")
    );
    for (value, expected) in [(1, "a"), (2, "b"), (3, "aa"), (7, "aaa")] {
        assert_eq!(
            initial(&algo(System::Alphabetic), value).as_deref(),
            Some(expected)
        );
    }
    assert_eq!(
        initial(&algo(System::Symbolic), 120).unwrap(),
        "b".repeat(60)
    );
    assert_eq!(initial(&algo(System::Symbolic), 121), None);
    let add = Algo::Own(
        System::Additive,
        vec![],
        vec![(3, "a".into()), (2, "b".into()), (0, "z".into())],
    );
    assert_eq!(initial(&add, 5).as_deref(), Some("ab"));
    assert_eq!(initial(&add, 4), None);
    assert_eq!(initial(&add, 0).as_deref(), Some("z"));
}

#[test]
fn descriptors_apply_range_then_padding_then_negative() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut rules = HashMap::new();
    rules.insert(
        "n".into(),
        Raw {
            system: Some(System::Numeric),
            symbols: Some((0..10).map(|n| n.to_string()).collect()),
            negative: Some(("<".into(), ">".into())),
            pad: Some((5, "0".into())),
            range: Some(Some(vec![(-20, 20)])),
            fallback: Some("lower-alpha".into()),
            ..Raw::default()
        },
    );
    assert_eq!(generate(&rules, -12, "n", 0, vcx), "<012>");
    assert_eq!(generate(&rules, 21, "n", 0, vcx), "u");
    rules.insert(
        "arabic".into(),
        Raw {
            system: Some(System::Extends("arabic-indic".into())),
            ..Raw::default()
        },
    );
    assert_eq!(
        generate(&rules, i64::MIN, "arabic", 0, vcx),
        "-٩٢٢٣٣٧٢٠٣٦٨٥٤٧٧٥٨٠٨"
    );
}

#[test]
fn disclosure_context_survives_extends_and_fallback() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut rules = HashMap::new();
    rules.insert(
        "alias".into(),
        Raw {
            system: Some(System::Extends("disclosure-closed".into())),
            ..Raw::default()
        },
    );
    rules.insert(
        "limited".into(),
        Raw {
            system: Some(System::Fixed(1)),
            symbols: Some(vec!["x".into()]),
            fallback: Some("alias".into()),
            ..Raw::default()
        },
    );
    let mut style = {
        let mut with_inherited = crate::style::computed::Computed {
            ..Default::default()
        };
        with_inherited.rtl = Some(true);
        with_inherited
    };
    let rtl = Context::new(&style, Context::default());
    assert_eq!(super::generate(&rules, 2, "alias", 0, rtl, vcx), "◂");
    assert_eq!(super::generate(&rules, 2, "limited", 0, rtl, vcx), "◂");
    style.writing_mode = Some(crate::style::computed::writing_mode::WritingModeKw::new(
        true,
        style.wm().block_rl(),
        style.wm().sideways(),
    ));
    let vertical = Context::new(&style, Context::default());
    assert_eq!(super::generate(&rules, 2, "limited", 0, vertical, vcx), "▴");
    assert_eq!(super::generate(&rules, -2, "alias", 0, vertical, vcx), "▴");
}

#[test]
fn extended_builtin_cyclic_styles_accept_negative_values() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    for (name, expected) in [("disc", "•"), ("circle", "◦"), ("square", "▪")] {
        let rules = HashMap::from([(
            "alias".into(),
            Raw {
                system: Some(System::Extends(name.into())),
                ..Raw::default()
            },
        )]);
        assert_eq!(generate(&rules, i64::MIN, "alias", 0, vcx), expected);
        assert_eq!(generate(&rules, 0, "alias", 0, vcx), expected);
    }
}

#[test]
fn builtin_fallback_keeps_document_style_overrides() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let rules = HashMap::from([
        (
            "limited".into(),
            Raw {
                system: Some(System::Fixed(1)),
                symbols: Some(vec!["x".into()]),
                fallback: Some("cjk-earthly-branch".into()),
                ..Raw::default()
            },
        ),
        (
            "cjk-decimal".into(),
            Raw {
                system: Some(System::Numeric),
                symbols: Some(vec!["o".into(), "i".into()]),
                ..Raw::default()
            },
        ),
    ]);
    assert_eq!(generate(&rules, 12, "limited", 0, vcx), "亥");
    assert_eq!(generate(&rules, 13, "limited", 0, vcx), "iioi");
    assert_eq!(generate(&rules, -13, "limited", 0, vcx), "-iioi");
}
