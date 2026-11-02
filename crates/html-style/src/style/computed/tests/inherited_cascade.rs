//! Каскад наследуемых длинных свойств реестра: сокращения `list-style`,
//! `text-align`, `text-box`, `line-clamp` раскрыты разбором, общие слова
//! решает реестр (`inherit` — пустые свои поля, `initial` — явное начальное
//! значение), негодная подстановка — `unset`, каждое свойство пишет только
//! свои поля.

use crate::style::computed::MarkerType::{Counter, Text};
use crate::style::computed::{BoxStyle, Computed, MarkerType, TextAlign};
use crate::style::css::{DeclBlock, Media, VarMap, parse_decls_in};
use crate::style::values::value::{Color, Len};
use crate::values::DocumentValues;

fn style_with(css: &str, vars: &VarMap) -> Computed {
    let doc = DocumentValues::detached(Media::default());
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
    assert_eq!(c.inh.table_list.list_style_type, Some(MarkerType::None));
    assert_eq!(c.reset_generated.no_marker, None);
    assert_eq!(c.inh.table_list.list_style_image, Some(None));
    assert_eq!(c.inh.table_list.list_style_inside, Some(false));
    let c = style("list-style: none square inside");
    assert_eq!(
        c.inh.table_list.list_style_type,
        Some(Counter("square".into()))
    );
    assert_eq!(c.inh.table_list.list_style_inside, Some(true));
    assert_eq!(c.inh.table_list.list_style_image, Some(None));
    // A second `none` has no free component: the declaration is dropped.
    let c = style("list-style-type: circle; list-style: none none square");
    assert_eq!(
        c.inh.table_list.list_style_type,
        Some(Counter("circle".into()))
    );
    let c = style("list-style: url(m.png)");
    assert_eq!(
        c.inh.table_list.list_style_type,
        Some(Counter("disc".into()))
    );
    assert!(matches!(c.inh.table_list.list_style_image, Some(Some(_))));
    let c = style("list-style: '→ ' inside");
    assert_eq!(c.inh.table_list.list_style_type, Some(Text("→ ".into())));
    assert_eq!(c.reset_generated.marker_text, None);
    assert_eq!(c.reset_generated.no_marker, None);
    // Order of the shorthand and its longhand in one block: by position.
    let c = style("list-style: square; list-style-type: decimal");
    assert_eq!(
        c.inh.table_list.list_style_type,
        Some(Counter("decimal".into()))
    );
    let c = style("list-style-type: decimal; list-style: square");
    assert_eq!(
        c.inh.table_list.list_style_type,
        Some(Counter("square".into()))
    );
}

#[test]
fn inherit_empties_own_fields_and_initial_is_explicit() {
    let c = style("list-style-type: square; list-style-type: inherit");
    assert_eq!(c.inh.table_list.list_style_type, None);
    let c = style("list-style-type: square; list-style-type: unset");
    assert_eq!(c.inh.table_list.list_style_type, None);
    let c = style("list-style-type: square; list-style-type: initial");
    assert_eq!(
        c.inh.table_list.list_style_type,
        Some(Counter("disc".into()))
    );
    let c = style("visibility: hidden; visibility: initial");
    assert_eq!(
        (c.inh.ui.hidden, c.reset_context.collapsed),
        (Some(false), Some(false))
    );
    let c = style("text-indent: 5px hanging; text-indent: initial");
    assert_eq!(c.inh.text_flow.text_indent, Some(Len::Px(0.0)));
    assert_eq!(c.inh.text_flow.text_indent_hanging, Some(false));
    let c = style("color: red; color: initial");
    assert_eq!(
        c.inh.ui.color,
        Color::parse(
            "canvastext",
            DocumentValues::detached(Media::default()).values()
        )
    );
    assert!(c.reset_cascade.color_initial);
    let c = style("color: initial; color: inherit");
    assert_eq!(c.inh.ui.color, None);
    assert!(!c.reset_cascade.color_initial);
}

#[test]
fn invalid_substitution_is_unset() {
    let mut vars = VarMap::new();
    vars.insert("--v".into(), "bogus".into());
    let c = style_with("visibility: hidden; visibility: var(--v)", &vars);
    assert_eq!(c.inh.ui.hidden, None);
    let c = style_with("color: red; color: var(--v)", &vars);
    assert_eq!(c.inh.ui.color, None);
    vars.insert("--v".into(), "square inside".into());
    let c = style_with("list-style: var(--v)", &vars);
    assert_eq!(
        c.inh.table_list.list_style_type,
        Some(Counter("square".into()))
    );
    assert_eq!(c.inh.table_list.list_style_inside, Some(true));
}

#[test]
fn invalid_values_are_dropped_at_parse() {
    let c = style("cursor: pointer; cursor: bogus");
    assert_eq!(c.inh.ui.cursor.as_deref(), Some("pointer"));
    let c = style("tab-size: 2.5; tab-size: -1");
    assert_eq!(c.inh.text_flow.tab_size, Some(2.5));
    let c = style("border-spacing: 2px; border-spacing: thin");
    assert_eq!(
        c.inh.table_list.border_spacing,
        Some((Some(Len::Px(2.0)), Some(Len::Px(2.0))))
    );
    let c = style("text-transform: uppercase; text-transform: uppercase lowercase");
    assert!(c.inh.text_flow.text_transform.is_some());
    assert_eq!(c.inh.text_flow.text_transform_flags, 0);
}

#[test]
fn text_align_is_a_shorthand_of_all_and_last() {
    let c = style("text-align: justify-all");
    assert_eq!(c.inh.text_flow.text_align, Some(TextAlign::Justify));
    assert_eq!(
        c.inh.text_flow.text_align_last,
        Some(Some(TextAlign::Justify))
    );
    // css-text-4 §7.1: other values reset `text-align-last` to `auto`.
    let c = style("text-align-last: right; text-align: center");
    assert_eq!(c.inh.text_flow.text_align, Some(TextAlign::Center));
    assert_eq!(c.inh.text_flow.text_align_last, Some(None));
    let c = style("text-align: center; text-align-last: right");
    assert_eq!(
        c.inh.text_flow.text_align_last,
        Some(Some(TextAlign::Right))
    );
    let c = style("text-align: match-parent");
    assert_eq!(c.reset_text_box.text_align_match_parent, 3);
    assert!(style("text-align: auto").inh.text_flow.text_align.is_none());
}

mod values;
