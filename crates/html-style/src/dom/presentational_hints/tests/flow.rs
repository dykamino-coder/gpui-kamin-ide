//! Проверки наследования и представления.
use super::*;
#[test]
fn author_resets_and_nearest_table_control_cell_hints() {
    let nodes = crate::dom::parse(
        "<!doctype html><style>table{border-spacing:initial}td{padding-top:initial}</style><table cellspacing=10 cellpadding=10><tr><td></td></tr></table>",
        "",
    );
    // css-tables-3 §border-spacing: the initial value is 0 (inherited
    // property, so the initial value is stored explicitly).
    let spacing = find(&nodes, "table")
        .unwrap()
        .style
        .cascaded()
        .inh
        .table_list
        .border_spacing;
    assert_eq!(spacing, Some((Some(Len::Px(0.0)), Some(Len::Px(0.0)))));
    let padding = find(&nodes, "td")
        .unwrap()
        .style
        .cascaded()
        .reset_box_layout
        .padding;
    // css-box-4 §4.1: the initial value of a padding side is 0.
    assert_eq!(padding.top, Some(Len::Px(0.0)));
    assert_eq!(padding.bottom, Some(Len::Px(10.0)));

    let nodes = crate::dom::parse(
        "<!doctype html><table cellpadding=10><tr><td><table cellpadding=3><tr><th></th></tr></table></td></tr></table>",
        "",
    );
    assert_eq!(
        find(&nodes, "td")
            .unwrap()
            .style
            .cascaded()
            .reset_box_layout
            .padding
            .top,
        Some(Len::Px(10.0))
    );
    assert_eq!(
        find(&nodes, "th")
            .unwrap()
            .style
            .cascaded()
            .reset_box_layout
            .padding
            .top,
        Some(Len::Px(3.0))
    );
}

/// CSS Cascade 5 §7.3 and HTML rendering §15.3.8: a CSS-wide keyword of the
/// author origin beats the table hint of the same longhand and leaves the
/// other longhands of the hint shorthand intact.
#[test]
fn css_wide_keywords_override_table_hints() {
    let table = "<table cellspacing=10 cellpadding=10 width=200 height=50 bgcolor=red align=center><tr><td></td></tr></table>";
    for keyword in ["initial", "unset"] {
        let html = format!(
            "<!doctype html><style>table{{border-spacing:{keyword};width:{keyword};height:{keyword};background-color:{keyword};margin-left:{keyword}}}td{{padding-top:{keyword};padding-left:{keyword}}}</style>{table}"
        );
        let doc = crate::dom::parse(&html, "");
        let nodes = &doc.nodes;
        // The initial value, explicit or as the empty slot.
        let auto = |l: Option<Len>| matches!(l, None | Some(Len::Auto));
        let zero = |l: Option<Len>| matches!(l, None | Some(Len::Px(0.0)));
        let t = &find(nodes, "table").unwrap().style;
        // `initial` — explicit zero; `unset` of an inherited property —
        // inheritance (the empty slot).
        assert!(
            matches!(
                t.cascaded().inh.table_list.border_spacing,
                None | Some((Some(Len::Px(0.0)), Some(Len::Px(0.0))))
            ),
            "{keyword}"
        );
        assert!(
            auto(t.cascaded().reset_box_layout.width) && auto(t.cascaded().reset_box_layout.height),
            "{keyword}"
        );
        assert!(
            t.cascaded()
                .reset_background
                .background
                .is_none_or(|c| c.a == 0.0),
            "{keyword}"
        );
        assert!(zero(t.cascaded().reset_box_layout.margin.left), "{keyword}");
        assert_eq!(
            t.cascaded().reset_box_layout.margin.right,
            Some(Len::Auto),
            "{keyword}"
        );
        let td = &find(nodes, "td").unwrap().style;
        // css-box-4 §4.1: the initial value of a padding side is 0.
        assert_eq!(
            td.cascaded().reset_box_layout.padding.top,
            Some(Len::Px(0.0)),
            "{keyword}"
        );
        assert_eq!(
            td.cascaded().reset_box_layout.padding.left,
            Some(Len::Px(0.0)),
            "{keyword}"
        );
        assert_eq!(
            td.cascaded().reset_box_layout.padding.bottom,
            Some(Len::Px(10.0)),
            "{keyword}"
        );
    }

    // `inherit` takes the parent's value instead of the hint.
    let html = format!(
        "<!doctype html><style>div{{border-spacing:3px}}table{{border-spacing:inherit}}td{{padding-top:inherit}}</style><div>{table}</div>"
    );
    let doc = crate::dom::parse(&html, "");
    let nodes = &doc.nodes;
    // `border-spacing` is inherited: the own slot is empty (no hint value)
    // and the merge takes the parent's computed value.
    let spacing = find(nodes, "table")
        .unwrap()
        .style
        .cascaded()
        .inh
        .table_list
        .border_spacing;
    assert_eq!(spacing, None);
    let td = &find(nodes, "td").unwrap().style;
    assert!(
        td.cascaded()
            .reset_cascade
            .inherit_longhands
            .contains(crate::style::css::LonghandId::PaddingTop)
    );
    assert_eq!(
        td.cascaded().reset_box_layout.padding.bottom,
        Some(Len::Px(10.0))
    );
}
