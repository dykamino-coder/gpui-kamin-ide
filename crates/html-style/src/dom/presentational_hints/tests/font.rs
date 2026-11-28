//! Legacy font hint parsing, inheritance and author cascade precedence.

use super::{Len, find};

#[test]
fn legacy_font_sizes_use_absolute_table_and_saturating_relative_offsets() {
    for (raw, expected) in [
        ("1", 10.0),
        ("2", 13.0),
        ("3", 16.0),
        ("4", 18.0),
        ("5", 24.0),
        ("6", 32.0),
        ("7", 48.0),
        ("0", 10.0),
        ("9999999999999999999", 48.0),
        ("+1", 18.0),
        ("-1", 13.0),
        ("-999999999999", 10.0),
        (" +2tail", 24.0),
    ] {
        let html = format!(
            "<!doctype html><div style='font-size:40px'><font size='{raw}'><span>text</span></font></div>"
        );
        let nodes = parse(&html, "");
        assert_eq!(
            find(&nodes, "span")
                .unwrap()
                .style
                .computed()
                .inh
                .font
                .font_size,
            Some(Len::Px(expected)),
            "{raw}"
        );
    }
}

#[test]
fn font_hints_inherit_and_author_css_overrides_each_attribute() {
    for direction in ["ltr", "rtl"] {
        let nodes = parse(
            &format!(
                "<!doctype html><font face='sans-serif' size=2 color=red dir={direction}><span>text</span></font>"
            ),
            "",
        );
        let style = find(&nodes, "span").unwrap().style.computed();
        assert_eq!(
            style.inh.font.font_families,
            find(&nodes, "font")
                .unwrap()
                .style
                .computed()
                .inh
                .font
                .font_families
        );
        assert!(style.inh.font.font_families.is_some());
        assert_eq!(style.inh.font.font_size, Some(Len::Px(13.0)));
        let reference = parse("<span style='color:red'>text</span>", "");
        assert_eq!(
            style.inh.ui.color,
            find(&reference, "span")
                .unwrap()
                .style
                .computed()
                .inh
                .ui
                .color
        );
        let nodes = parse(
            &format!(
                "<!doctype html><style>font{{font-family:monospace;font-size:20px;color:blue}}</style><font face='sans-serif' size=2 color=red dir={direction}><span>text</span></font>"
            ),
            "",
        );
        let style = find(&nodes, "span").unwrap().style.computed();
        assert_eq!(style.inh.font.font_size, Some(Len::Px(20.0)));
        let reference = parse("<span style='font-family:monospace'>text</span>", "");
        assert_eq!(
            style.inh.font.font_families,
            find(&reference, "span")
                .unwrap()
                .style
                .computed()
                .inh
                .font
                .font_families
        );
        let reference = parse("<span style='color:blue'>text</span>", "");
        assert_eq!(
            style.inh.ui.color,
            find(&reference, "span")
                .unwrap()
                .style
                .computed()
                .inh
                .ui
                .color
        );
    }
}

#[test]
fn invalid_font_attributes_do_not_inject_css_or_reset_inheritance() {
    for (face, size) in [
        ("inherit", "junk"),
        ("serif;color:red", "+"),
        ("sans-serif !important", "- 2"),
    ] {
        let html = format!(
            "<!doctype html><div style='font-family:monospace;font-size:21px;color:green'><font face='{face}' size='{size}'><span>text</span></font></div>"
        );
        let nodes = parse(&html, "");
        let parent = find(&nodes, "div").unwrap().style.computed();
        let child = find(&nodes, "span").unwrap().style.computed();
        assert_eq!(child.inh.font.font_size, parent.inh.font.font_size);
        assert_eq!(child.inh.font.font_families, parent.inh.font.font_families);
        assert_eq!(child.inh.ui.color, parent.inh.ui.color);
    }
}

fn parse(html: &str, css: &str) -> crate::dom::Parsed {
    let parsed = crate::dom::parse(html, css);
    crate::style::cascade::materialize::materialize(
        &parsed.nodes,
        &std::rc::Rc::new(crate::style::computed::Computed::default()),
        parsed.values(),
    );
    parsed
}
