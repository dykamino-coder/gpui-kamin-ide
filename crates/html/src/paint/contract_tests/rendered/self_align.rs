//! `align-self` reaches only flex items, grid items and absolutely-positioned
//! boxes (css-align-3 §6.1); block layout keeps its own placement of a
//! block-level box whatever the author wrote.

use super::quads;

/// The left edge of the green box.
fn green_x(html: &str) -> f32 {
    let found = quads(html, 200., 100.);
    let green: Vec<_> = found
        .iter()
        .filter(|(_, c)| c.to_rgb() == gpui::rgb(0x008000))
        .map(|(b, _)| f32::from(b.origin.x))
        .collect();
    assert_eq!(green.len(), 1, "one green box in {html}");
    green[0]
}

fn page(container: &str, item: &str) -> String {
    format!(
        "<style>body{{margin:0}} i{{display:block;width:50px;height:10px;\
         background:green}}</style><div style='{container}'><i style='{item}'></i></div>"
    )
}

#[test]
fn a_block_in_block_flow_ignores_the_authored_align_self() {
    for item in [
        "",
        "align-self:flex-end",
        "align-self:center",
        "align-self:end",
    ] {
        assert_eq!(green_x(&page("", item)), 0., "{item}");
    }
}

#[test]
fn block_layout_keeps_its_rtl_placement_under_an_authored_align_self() {
    // An overflow-capable fixed-width block of rtl flow stands at the right
    // edge (CSS 2.1 §10.3.3); `align-self` does not move it.
    for item in ["", "align-self:flex-start", "align-self:center"] {
        assert_eq!(green_x(&page("direction:rtl", item)), 150., "{item}");
    }
}

#[test]
fn a_flex_item_uses_the_authored_align_self() {
    let column = "display:flex;flex-direction:column";
    assert_eq!(green_x(&page(column, "align-self:flex-end")), 150.);
    assert_eq!(green_x(&page(column, "align-self:center")), 75.);
    let inherited = format!("{column};align-self:flex-end");
    assert_eq!(green_x(&page(&inherited, "align-self:inherit")), 150.);
}
