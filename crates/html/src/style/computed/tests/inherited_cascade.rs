//! Каскад наследуемых длинных свойств реестра: сокращения `list-style`,
//! `text-align`, `text-box`, `line-clamp` раскрыты разбором, общие слова
//! решает реестр (`inherit` — пустые свои поля, `initial` — явное начальное
//! значение), негодная подстановка — `unset`, каждое свойство пишет только
//! свои поля.

use crate::document::state::DocState;
use crate::style::computed::{Computed, TextAlign};
use crate::style::css::{DeclBlock, Media, VarMap, parse_decls_in};
use crate::style::values::value::{Color, Len};

fn style_with(css: &str, vars: &VarMap) -> Computed {
    let doc = DocState::new(Media::default());
    let block: DeclBlock = parse_decls_in(css, doc.values());
    let mut c = Computed::default();
    c.apply_decls_with_vars(&block, vars, doc.values());
    c
}

fn style(css: &str) -> Computed {
    style_with(css, &VarMap::new())
}

#[test]
fn list_style_assigns_none_to_the_unset_components() {
    let c = style("list-style: none");
    assert_eq!(c.list_style_type.as_deref(), Some("none"));
    assert_eq!(c.no_marker, Some(true));
    assert_eq!(c.list_style_image, Some(None));
    assert_eq!(c.list_style_inside, Some(false));
    let c = style("list-style: none square inside");
    assert_eq!(c.list_style_type.as_deref(), Some("square"));
    assert_eq!(c.list_style_inside, Some(true));
    assert_eq!(c.list_style_image, Some(None));
    // A second `none` has no free component: the declaration is dropped.
    let c = style("list-style-type: circle; list-style: none none square");
    assert_eq!(c.list_style_type.as_deref(), Some("circle"));
    let c = style("list-style: url(m.png)");
    assert_eq!(c.list_style_type.as_deref(), Some("disc"));
    assert!(matches!(c.list_style_image, Some(Some(_))));
    let c = style("list-style: '→ ' inside");
    assert_eq!(c.marker_text.as_deref(), Some("→ "));
    assert_eq!(c.no_marker, Some(false));
    // Order of the shorthand and its longhand in one block: by position.
    let c = style("list-style: square; list-style-type: decimal");
    assert_eq!(c.list_style_type.as_deref(), Some("decimal"));
    let c = style("list-style-type: decimal; list-style: square");
    assert_eq!(c.list_style_type.as_deref(), Some("square"));
}

#[test]
fn inherit_empties_own_fields_and_initial_is_explicit() {
    let c = style("list-style-type: square; list-style-type: inherit");
    assert_eq!(c.list_style_type, None);
    let c = style("list-style-type: square; list-style-type: unset");
    assert_eq!(c.list_style_type, None);
    let c = style("list-style-type: square; list-style-type: initial");
    assert_eq!(c.list_style_type.as_deref(), Some("disc"));
    let c = style("visibility: hidden; visibility: initial");
    assert_eq!((c.hidden, c.collapsed), (Some(false), Some(false)));
    let c = style("text-indent: 5px hanging; text-indent: initial");
    assert_eq!(c.text_indent, Some(Len::Px(0.0)));
    assert_eq!(c.text_indent_hanging, Some(false));
    let c = style("color: red; color: initial");
    assert_eq!(
        c.color,
        Color::parse("canvastext", DocState::new(Media::default()).values())
    );
    assert!(c.color_initial);
    let c = style("color: initial; color: inherit");
    assert_eq!(c.color, None);
    assert!(!c.color_initial);
}

#[test]
fn invalid_substitution_is_unset() {
    let mut vars = VarMap::new();
    vars.insert("--v".into(), "bogus".into());
    let c = style_with("visibility: hidden; visibility: var(--v)", &vars);
    assert_eq!(c.hidden, None);
    let c = style_with("color: red; color: var(--v)", &vars);
    assert_eq!(c.color, None);
    vars.insert("--v".into(), "square inside".into());
    let c = style_with("list-style: var(--v)", &vars);
    assert_eq!(c.list_style_type.as_deref(), Some("square"));
    assert_eq!(c.list_style_inside, Some(true));
}

#[test]
fn invalid_values_are_dropped_at_parse() {
    let c = style("cursor: pointer; cursor: bogus");
    assert_eq!(c.cursor.as_deref(), Some("pointer"));
    let c = style("tab-size: 2.5; tab-size: -1");
    assert_eq!(c.tab_size, Some(2.5));
    let c = style("border-spacing: 2px; border-spacing: thin");
    assert_eq!(
        c.border_spacing,
        Some((Some(Len::Px(2.0)), Some(Len::Px(2.0))))
    );
    let c = style("text-transform: uppercase; text-transform: uppercase lowercase");
    assert!(c.text_transform.is_some());
    assert_eq!(c.text_transform_flags, 0);
}

#[test]
fn text_align_is_a_shorthand_of_all_and_last() {
    let c = style("text-align: justify-all");
    assert_eq!(c.text_align, Some(TextAlign::Justify));
    assert_eq!(c.text_align_last, Some(Some(TextAlign::Justify)));
    // css-text-4 §7.1: other values reset `text-align-last` to `auto`.
    let c = style("text-align-last: right; text-align: center");
    assert_eq!(c.text_align, Some(TextAlign::Center));
    assert_eq!(c.text_align_last, Some(None));
    let c = style("text-align: center; text-align-last: right");
    assert_eq!(c.text_align_last, Some(Some(TextAlign::Right)));
    let c = style("text-align: match-parent");
    assert_eq!(c.text_align_match_parent, 3);
    assert!(style("text-align: auto").text_align.is_none());
}

#[test]
fn word_break_keeps_its_own_fields() {
    let c = style("overflow-wrap: normal; word-break: break-word");
    assert_eq!(c.break_word, Some(false));
    assert_eq!(c.wrap_anywhere, Some(false));
    assert_eq!(c.word_break_word, Some(true));
    let c = style("line-break: anywhere");
    assert_eq!(c.break_anywhere, None);
    assert_eq!(c.break_anywhere_strict, Some(true));
}

#[test]
fn line_clamp_expands_to_max_lines_ellipsis_and_continue() {
    let c = style("line-clamp: 2");
    assert_eq!(c.line_clamp, Some(2));
    assert_eq!(c.clamp_mark, Some(None));
    assert_eq!(c.clamp_auto, Some(false));
    assert_eq!(
        (c.clamp_continue, c.clamp_legacy),
        (Some(true), Some(false))
    );
    assert!(!c.clamp_by_height());
    let c = style("line-clamp: auto");
    assert_eq!(c.line_clamp, None);
    assert!(c.clamp_by_height());
    // Lines and the box height both clamp (`line-clamp-041`).
    let c = style("line-clamp: 4 auto");
    assert_eq!(c.line_clamp, Some(4));
    assert!(c.clamp_by_height());
    let c = style("line-clamp: auto no-ellipsis");
    assert_eq!(c.used_block_ellipsis().as_deref(), Some(""));
    assert!(c.clamp_by_height());
    // `continue: collapse` alone: `max-lines` is initially `auto`.
    assert!(style("continue: collapse").clamp_by_height());
    let c = style("line-clamp: 3 'x' -webkit-legacy");
    assert_eq!(c.clamp_mark, Some(Some("x".into())));
    assert_eq!(c.clamp_legacy, Some(true));
    let c = style("line-clamp: none");
    assert_eq!(c.used_block_ellipsis().as_deref(), Some(""));
    let c = style("-webkit-line-clamp: 4");
    assert_eq!(c.line_clamp, Some(4));
    assert_eq!(c.used_block_ellipsis(), None);
    assert_eq!(c.clamp_legacy, Some(true));
    // `block-ellipsis` is inherited: the root default is `no-ellipsis`.
    assert_eq!(
        Computed::default().used_block_ellipsis().as_deref(),
        Some("")
    );
}

#[test]
fn text_box_shorthand_defaults_to_trim_both() {
    use crate::style::computed::TextEdge;
    let c = style("text-box: cap alphabetic");
    assert!(c.text_box_trim_start && c.text_box_trim_end);
    assert_eq!(
        (c.text_box_over, c.text_box_under),
        (TextEdge::Cap, TextEdge::Alphabetic)
    );
    let c = style("text-box: trim-start");
    assert!(c.text_box_trim_start && !c.text_box_trim_end);
    assert!(c.text_box_edge_set);
    let c = style("text-box: normal");
    assert!(!c.text_box_trim_start && !c.text_box_trim_end);
    for invalid in ["none", "cap trim-start alphabetic", "trim-start trim-end"] {
        let c = style(&format!("text-box: trim-end; text-box: {invalid}"));
        assert!(!c.text_box_trim_start && c.text_box_trim_end, "{invalid}");
    }
}

#[test]
fn tab_size_number_and_length_replace_each_other() {
    let c = style("tab-size: 40px; tab-size: 2.5");
    assert_eq!((c.tab_size, c.tab_size_len), (Some(2.5), None));
    let c = style("tab-size: 2.5; tab-size: 40px");
    assert_eq!((c.tab_size, c.tab_size_len), (None, Some(Len::Px(40.0))));
    for invalid in ["-1", "-1px", "10%", "auto", "NaN", "inf", "6.", "1e99"] {
        let c = style(&format!("tab-size: 2.5; tab-size: {invalid}"));
        assert_eq!((c.tab_size, c.tab_size_len), (Some(2.5), None), "{invalid}");
    }
}

#[test]
fn user_select_is_not_inherited_and_auto_follows_the_parent_box() {
    use crate::style::cascade::inherit::inherit;
    use crate::style::computed::UserSelect;
    let doc = DocState::new(Media::default());
    let vcx = doc.values();
    let none = inherit(&Computed::default(), &style("user-select: none"), vcx);
    assert_eq!(none.user_select, Some(UserSelect::None));
    assert_eq!(none.no_select, Some(true));
    // css-ui-4 §6.1: the computed value is not inherited ...
    let child = inherit(&none, &Computed::default(), vcx);
    assert_eq!(child.user_select, None);
    // ... but the used value of `auto` under a `none` box is `none`.
    assert_eq!(child.no_select, Some(true));
    assert_eq!(
        inherit(&none, &style("user-select: auto"), vcx).no_select,
        Some(true)
    );
    assert_eq!(
        inherit(&none, &style("user-select: text"), vcx).no_select,
        Some(false)
    );
    let all = inherit(&Computed::default(), &style("user-select: all"), vcx);
    assert_eq!(
        inherit(&all, &style("user-select: auto"), vcx).no_select,
        Some(false)
    );
    // `inherit` takes the parent's computed keyword.
    let explicit = inherit(&all, &style("user-select: inherit"), vcx);
    assert_eq!(explicit.user_select, Some(UserSelect::All));
    assert_eq!(
        style("-webkit-user-select: contain").user_select,
        Some(UserSelect::Contain)
    );
}
