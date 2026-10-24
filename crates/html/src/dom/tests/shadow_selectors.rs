//! Тесты селекторов в теневом дереве: `::slotted(:has())` смотрит в поддерево
//! распределённого элемента, а не в свой fallback; `:host(:has())` и
//! `:host:has()` разделяют светлых и теневых потомков; отметки `:has()`
//! ставятся до переписи всех деревьев; безликий хост внутри `:not()`/`:is()`
//! требует годного предмета.

use super::*;
use crate::style::values::value::Color;

#[test]
fn shadow_fast_path_uses_normalized_html_attribute_names() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let green = Color::parse("green", doc.values());
    for attr in ["shadowrootmode", "SHADOWROOTMODE", "ShAdOwRoOtMoDe"] {
        for mode in ["open", "CLOSED"] {
            let html = format!(
                "<!doctype html><div><template {attr}='{mode}'><style>\
                 ::slotted(i) {{color:green}}\
                 </style><slot id=box></slot></template><i>assigned</i></div>"
            );
            assert_eq!(child_colors(&html), [green], "{attr}={mode}");
        }
    }
}

#[test]
fn shadow_fast_path_finds_nested_uppercase_templates() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let colors = child_colors(
        "<!doctype html><main><template SHADOWROOTMODE=open>\
         <div><template SHADOWROOTMODE=open><style>i{color:green}</style>\
         <div id=box><i></i></div></template></div></template></main>",
    );
    assert_eq!(colors, [Color::parse("green", doc.values())]);
}

#[test]
fn slotted_has_matches_the_assigned_elements_own_subtree() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let colors = child_colors(
        r#"<!doctype html><style>i { color: red }</style><div><template shadowrootmode=open><style>
          i { color: red } slot { color: red }
          section > slot::slotted(.Assigned:has(.Child)) { color: green }
          aside slot::slotted(*) { color: red }
          ::slotted(:has(section)) { color: red }
          ::slotted(*) i { color: red }
        </style><section><slot></slot></section></template>
        <div id=box><i class=Assigned><b class=Child></b></i></div></div>"#,
    );
    // The assigned element is #box, so its descendant i does not match ::slotted.
    assert_eq!(colors, vec![Color::parse("red", vcx)]);
    let colors = child_colors(
        r#"<!doctype html><div><template shadowrootmode=open><style>
          i { color: red } slot { color: red }
          section > slot::slotted(.Assigned:has(.Child)) { color: green }
          aside slot::slotted(*) { color: red }
          ::slotted(:has(section)) { color: red }
        </style><section><slot id=box></slot></section></template>
        <i class=Assigned><b class=Child></b></i></div>"#,
    );
    assert_eq!(colors, vec![Color::parse("green", vcx)]);
}

#[test]
fn slotted_does_not_select_its_own_fallback() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let colors = child_colors(
        r#"<!doctype html><div><template shadowrootmode=open><style>
          i { color: red }
          ::slotted(i:has(b)) { color: green }
        </style><slot id=box><i><b></b></i></slot></template></div>"#,
    );
    assert_eq!(colors, vec![Color::parse("red", vcx)]);
}

#[test]
fn host_has_keeps_light_and_shadow_descendants_separate() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let colors = child_colors(
        r#"<!doctype html><div><template shadowrootmode=open>
        <style>
          i { color: red }
          :host(:has(section > .Heading)) .light { color: green }
          :host:has(article > h2) .shadow { color: green }
          :host(:has(article)) .shadow { color: red }
          :host:has(section) .light { color: red }
        </style><div id=box><i class=light></i><i class=shadow></i></div>
        <article><h2></h2></article></template><section><h1 class=Heading></h1></section></div>"#,
    );
    assert_eq!(colors, vec![Color::parse("green", vcx); 2]);
}

#[test]
fn host_has_registration_precedes_all_tree_passports() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let colors = child_colors(
        r#"<!doctype html><main><template shadowrootmode=open>
        <div><template shadowrootmode=open><style>
          i { color: red }
          :host(:has(section)) i { color: green }
        </style><div id=box><i></i></div></template><section></section></div>
        </template></main>"#,
    );
    assert_eq!(colors, vec![Color::parse("green", vcx)]);
}

#[test]
fn featureless_negation_requires_an_eligible_subject() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let colors = child_colors(
        r#"<!doctype html><div id=box><template shadowrootmode=open>
        <style>i { color: red }
          :is(aside, :host) .a { color: green }
          :not(:not(:host)) .b { color: green }
          :not(aside) i { color: red }
          :not(.foo:host) i { color: red }
          :not(:host > .foo) i { color: red }
          :is(:not(body) > :host) i { color: red }
          :host:not(body) i { color: red }
        </style><i class=a></i><i class=b></i>
        </template></div>"#,
    );
    assert_eq!(colors, vec![Color::parse("green", vcx); 2]);
}
