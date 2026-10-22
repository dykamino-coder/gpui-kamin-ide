//! Каскад текстовых сокращений: `white-space`, `text-wrap`,
//! `text-decoration`, `text-emphasis` раскрыты разбором, и итог не зависит от
//! порядка объявлений и от соседних свойств (`textarea-pre-wrap-*`: четыре
//! `initial` рядом с `white-space: pre-wrap` поверх `font` агента).

use crate::document::state::{DocState, ValueCx};
use crate::style::computed::Computed;
use crate::style::css::{LinkMatch, Media, Origin, Rule, Selector, parse_decls_in};
use crate::style::values::value::Len;

fn rule(css: &str, origin: Origin, order: usize, vcx: ValueCx<'_>) -> Rule {
    Rule {
        sel: Selector::parse("textarea").unwrap(),
        decls: parse_decls_in(css, vcx),
        order,
        origin,
        layer: vec![u32::MAX],
        link: LinkMatch::Any,
    }
}

type Flags = [Option<bool>; 7];

fn flags(c: &Computed) -> Flags {
    [
        c.break_word,
        c.wrap_anywhere,
        c.break_anywhere,
        c.keep_all,
        c.nowrap,
        c.keep_spaces,
        c.preserve_newlines,
    ]
}

fn author(white_space_first: bool) -> String {
    let resets = "word-wrap: initial; overflow-wrap: initial; line-break: initial; \
                  word-break: initial; font-size: 20px; font-family: Ahem; line-height: 1em";
    if white_space_first {
        format!("white-space: pre-wrap; {resets}")
    } else {
        format!("{resets}; white-space: pre-wrap")
    }
}

#[test]
fn textarea_pre_wrap_block_does_not_depend_on_order_or_the_ua_font() {
    let doc = DocState::new(Media::default());
    let vcx = doc.values();
    let alone = flags(&Computed::resolve(
        &[&rule("white-space: pre-wrap", Origin::Author, 0, vcx)],
        &Default::default(),
        vcx,
    ));
    assert_eq!(alone[4], Some(false), "pre-wrap wraps");
    assert_eq!(alone[5], Some(true), "pre-wrap keeps spaces");
    assert_eq!(alone[6], Some(true), "pre-wrap keeps newlines");
    let ua = rule(
        "font: 13.3333px monospace; overflow: auto",
        Origin::UserAgent,
        0,
        vcx,
    );
    for first in [true, false] {
        let block = rule(&author(first), Origin::Author, 1, vcx);
        for rules in [vec![&block], vec![&ua, &block]] {
            let c = Computed::resolve(&rules, &Default::default(), vcx);
            let got = flags(&c);
            assert_eq!(
                &got[4..],
                &alone[4..],
                "first={first} rules={}",
                rules.len()
            );
            // Четыре `initial` — `normal`/`auto`: перенос внутри слова выключен.
            assert_eq!(&got[..4], &[Some(false); 4], "first={first}");
            assert_eq!(c.font_size, Some(Len::Px(20.0)));
            assert_eq!(c.line_height, Some(Len::Em(1.0)));
            assert_eq!(
                c.monospace,
                Some(false),
                "author family wins over the UA font"
            );
        }
    }
}

#[test]
fn white_space_and_its_longhands_cascade_by_position() {
    let doc = DocState::new(Media::default());
    let vcx = doc.values();
    let style = |css: &str| {
        Computed::resolve(
            &[&rule(css, Origin::Author, 0, vcx)],
            &Default::default(),
            vcx,
        )
    };
    let c = style("white-space: pre; text-wrap-mode: wrap");
    assert_eq!((c.nowrap, c.keep_spaces), (Some(false), Some(true)));
    let c = style("text-wrap-mode: wrap; white-space: pre");
    assert_eq!((c.nowrap, c.keep_spaces), (Some(true), Some(true)));
    let c = style("white-space: nowrap");
    assert_eq!((c.nowrap, c.keep_spaces), (Some(true), Some(false)));
    let c = style("white-space: break-spaces");
    assert_eq!(c.break_after_spaces, Some(true));
    let c = style("white-space: pre; white-space: pre pre");
    assert_eq!(c.nowrap, Some(true), "invalid shorthand dropped whole");
    let c = style("text-wrap: balance nowrap");
    assert_eq!((c.nowrap, c.balance_lines), (Some(true), Some(true)));
    let c = style("text-wrap: balance; text-wrap: nowrap");
    assert_eq!((c.nowrap, c.balance_lines), (Some(true), Some(false)));
}

#[test]
fn text_decoration_shorthand_resets_and_validates_whole() {
    let doc = DocState::new(Media::default());
    let vcx = doc.values();
    let style = |css: &str| {
        Computed::resolve(
            &[&rule(css, Origin::Author, 0, vcx)],
            &Default::default(),
            vcx,
        )
    };
    let c = style("text-decoration-style: wavy; text-decoration: underline red");
    assert_eq!(c.td_style, Some(crate::style::computed::DecorStyle::Solid));
    assert_eq!(c.underline, Some(true));
    let c = style("text-decoration: underline; text-decoration: diagonal");
    assert_eq!(c.underline, Some(true));
    let c = style("text-decoration: underline red overline");
    assert_eq!(c.underline, None, "line words must be contiguous");
    let c = style("text-emphasis: open red; text-emphasis-color: blue");
    assert_eq!(c.text_emphasis.as_deref(), Some("\u{25CB}"));
    assert!(c.emphasis_color.is_some());
}
