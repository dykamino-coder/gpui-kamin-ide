//! Class edits in a document with a declarative shadow tree
//! (`repaint::shadow_scope`): an edit no shadow scope sees restyles its light
//! elements in place, one a shadow sheet reads or that reaches a host
//! rebuilds, and every revision equals a fresh build.

use super::update;

fn page(classes: [&str; 3]) -> String {
    let [outside, wrap, light] = classes;
    format!(
        "<!doctype html><html><head><style>body{{margin:0}}.alt{{background:#eee}}\
         .k.alt{{color:#a00}}</style></head><body><p class='{outside}'>light</p>\
         <section class='{wrap}'><div id=host><template shadowrootmode=open><style>\
         :host(.on){{color:green}} b{{display:block}} ::slotted(.hot){{color:blue}}</style>\
         <b>shadow</b><slot></slot></template><i class='{light}'>slotted</i></div></section>\
         </body></html>"
    )
}

#[test]
fn a_light_edit_no_shadow_scope_sees_is_restyled_in_place() {
    let old = page(["", "", "k"]);
    for new in [page(["alt", "", "k"]), page(["", "alt", "k"])] {
        assert!(update(&old, &new, ""), "{new}");
        assert!(update(&new, &old, ""), "{new}");
    }
}

#[test]
fn an_edit_a_shadow_sheet_reads_or_that_reaches_a_host_rebuilds() {
    let old = page(["", "", "k"]);
    let host = old.replace("<div id=host>", "<div id=host class=on>");
    let hosted = page(["", "", "k alt"]);
    let slotted = page(["", "", "k hot"]);
    for new in [host, hosted, slotted] {
        assert!(!update(&old, &new, ""), "{new}");
        assert!(!update(&new, &old, ""), "{new}");
    }
}
