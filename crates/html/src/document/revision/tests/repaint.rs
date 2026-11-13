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
fn layout_values_backgrounds_and_unfollowed_selectors_rebuild() {
    for style in [
        "html.alt body{font-size:20px}",
        "html.alt body{font-weight:bold}",
        "html.alt body{background:#eee}",
        "html.alt body{color:#111;margin:1px}",
        ".alt + body{color:#111}",
        "html.alt p::before{content:'x'}",
        "html.alt li:first-child{color:#111}",
        ":is(.alt) body{color:#111}",
        "html.alt body{color:#111}p::after{content:attr(class)}",
    ] {
        let old = page(style, "");
        let new = old.replace("<html>", "<html class=\"alt\">");
        assert_eq!(both(&old, &new), (false, false), "{style}");
    }
}

#[test]
fn an_unwrapped_body_folds_its_colour_into_the_top_boxes_and_rebuilds() {
    let old = page(".box.hot{color:red}", "").replace(";background:#fff", "");
    let new = old.replace("class=box", "class=\"box hot\"");
    assert_eq!(both(&old, &new), (false, false));
}

#[test]
fn hover_layers_and_floats_below_the_restyled_element_rebuild() {
    let style = "html.alt body{color:#111}a:hover{color:red}";
    let old = page(style, "<a href=#x>l</a>");
    let new = old.replace("<html>", "<html class=\"alt\">");
    assert_eq!(both(&old, &new), (false, false));
}
