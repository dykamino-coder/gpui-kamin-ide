//! Смены классов, чей перерасчёт стиля меняет только наследуемый цвет
//! (`revision::repaint`): ревизия без сборки равна свежей сборке, а смена,
//! которая доходит до значений раскладки или до копий стиля вне каскада,
//! собирается заново и тоже равна свежей.

use super::update;

fn page(style: &str, body: &str) -> String {
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;color:#000;background:#fff}}{style}</style>\
         </head><body><h1>t</h1><div class=box><p>a <span>b</span></p><p class=own>c</p>\
         </div><ul><li>x</li></ul></body></html>"
    )
    .replace("<body>", &format!("<body>{body}"))
}

/// Обе стороны правки: добавить класс корню и снять его.
fn both(old: &str, new: &str) -> (bool, bool) {
    (update(old, new, ""), update(new, old, ""))
}

#[test]
fn an_ancestor_class_that_recolours_the_body_is_restyled_in_place() {
    let style = "html.alt body{color:#111}.own{color:#00f}";
    let old = page(style, "");
    let new = old.replace("<html>", "<html class=\"alt\">");
    assert_eq!(both(&old, &new), (true, true));
}

#[test]
fn a_class_on_the_element_itself_recolours_its_subtree_in_place() {
    let style = ".box.hot{color:red}";
    let old = page(style, "");
    let new = old.replace("class=box", "class=\"box hot\"");
    assert_eq!(both(&old, &new), (true, true));
}

#[test]
fn layout_values_backgrounds_and_generated_content_rebuild_the_boxes_of_their_element() {
    for style in [
        "html.alt body{font-size:20px}",
        "html.alt body{font-weight:bold}",
        "html.alt body{background:#eee}",
        "html.alt body{color:#111;margin:1px}",
        "html.alt p::before{content:'x'}",
        // `attr(class)` of `p`: the edited `html` generates no box that reads it.
        "html.alt body{color:#111}p::after{content:attr(class)}",
    ] {
        let old = page(style, "");
        let new = old.replace("<html>", "<html class=\"alt\">");
        assert_eq!(both(&old, &new), (true, true), "{style}");
    }
}

#[test]
fn local_logical_and_structural_selectors_recolour_in_place() {
    for style in [
        "html.alt li:first-child{color:#111}",
        ":is(.alt) body{color:#111}",
        "html.alt p:not(.own){color:#111}",
        "html.alt p:is(.own,:not(.own)){color:#111}",
        "html.alt p:where(:not(.own)){color:#111}",
        // Sibling, positional and complex arguments are followed (invalidation::targets).
        ".alt + body{color:#111}",
        "html.alt li:nth-child(1 of .own){color:#111}",
        "html.alt li:not(ul li){color:#111}",
    ] {
        let old = page(style, "");
        let new = old.replace("<html>", "<html class=\"alt\">");
        assert_eq!(both(&old, &new), (true, true), "{style}");
    }
}

#[test]
fn a_top_box_under_an_unwrapped_body_is_recoloured_in_place() {
    let old = page(".box.hot{color:red}", "").replace(";background:#fff", "");
    let new = old.replace("class=box", "class=\"box hot\"");
    assert_eq!(both(&old, &new), (true, true));
}

#[test]
fn restyled_elements_without_a_box_are_left_and_the_rest_recolours_in_place() {
    // `.x` matches inside a `display: none` subtree and a hidden `template`:
    // neither generates a box before or after the edit.
    let style = "html.alt body{color:#111}html.alt .x{color:red}.h{display:none}";
    let body = "<div class=h><p class=x>h</p></div><template><p class=x>t</p></template>";
    let old = page(style, body);
    let new = old.replace("<html>", "<html class=\"alt\">");
    assert_eq!(both(&old, &new), (true, true));
    // An element that gains its box rebuilds the boxes of its parent.
    let style = "html.alt body{color:#111}html.alt .h{display:block}.h{display:none}";
    let old = page(style, "<div class=h>h</div>");
    let new = old.replace("<html>", "<html class=\"alt\">");
    assert_eq!(both(&old, &new), (true, true));
}

#[test]
fn hover_layers_below_the_restyled_element_are_recoloured_in_place() {
    // A hover layer is a cascaded stage: it follows the new inherited colour.
    let style = "html.alt body{color:#111}a:hover{color:red}";
    let old = page(style, "<a href=#x>l</a>");
    let new = old.replace("<html>", "<html class=\"alt\">");
    assert_eq!(both(&old, &new), (true, true));
    // A first-letter layer below it is still rebuilt.
    let style = "html.alt body{color:#111}p::first-letter{color:red}";
    let old = page(style, "");
    let new = old.replace("<html>", "<html class=\"alt\">");
    assert_eq!(both(&old, &new), (false, false));
}

#[test]
fn has_and_sibling_rules_on_a_shape_class_equal_a_fresh_build() {
    let rows: String = (0..6)
        .map(|i| format!("<div class=r>row {i}</div>"))
        .collect();
    let page = |class: &str| {
        format!(
            "<!doctype html><html><head><style>html,body{{margin:0;background:#fff;color:#000}}\
             .r{{height:20px}} body:has(rect.h) .r:nth-child(odd){{background:#eee}}\
             svg:has(.h) ~ .r{{color:#e00}}</style></head><body>{rows}\
             <svg width=40 height=20><rect{class} width=30 height=10 /></svg>{rows}</body></html>"
        )
    };
    // `update` compares the revision with a fresh build either way.
    update(&page(""), &page(" class=h"), "");
    update(&page(" class=h"), &page(""), "");
}

#[test]
fn a_background_of_an_element_with_hover_rules_is_restyled_in_place() {
    // The hover layer of the element changes with its background: a paint
    // change of a cascaded stage of its own, written with the box's.
    let style = "html.alt .own{background:#eee}.own:hover{color:red}";
    let old = page(style, "");
    let new = old.replace("<html>", "<html class=\"alt\">");
    assert_eq!(both(&old, &new), (true, true));
}

#[test]
fn a_geometry_change_rebuilds_the_boxes_of_its_element_in_place() {
    use crate::paint_path::{DAMAGE_LAYOUT, REBOX_UNITS};
    use std::sync::atomic::Ordering::Relaxed;
    let style = "html.alt .own{width:50px;background:#eee}";
    let old = page(style, "");
    let new = old.replace("<html>", "<html class=\"alt\">");
    let (roots, units) = (DAMAGE_LAYOUT.load(Relaxed), REBOX_UNITS.load(Relaxed));
    assert_eq!(both(&old, &new), (true, true));
    assert!(DAMAGE_LAYOUT.load(Relaxed) >= roots + 2);
    assert!(REBOX_UNITS.load(Relaxed) >= units + 2);
}

#[test]
fn calc_values_of_restyled_elements_do_not_grow_the_document_arena() {
    let style = ".own{width:calc(50% + 4px)}.own.hot{color:red}";
    let old = page(style, "");
    let new = old.replace("class=own", "class=\"own hot\"");
    let mut doc = crate::document::Document::builder(&old, "")
        .build()
        .unwrap();
    doc.update(&new, "").unwrap();
    let after_one = doc.state.values.calc.records();
    for i in 0..6 {
        doc.update(if i % 2 == 0 { &old } else { &new }, "")
            .unwrap();
    }
    assert_eq!(doc.state.values.calc.records(), after_one);
}
