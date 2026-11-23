//! Independent attr() reads allow local restyle and match a fresh build.

use super::{both, page};

#[test]
fn unrelated_generated_attributes_allow_class_and_data_restyles() {
    for dependent in [
        "p::after{content:attr(title)}",
        r"p::after{content:a\74tr(\64 ata-label)}",
        "p::after{content:ATTR(title RAW-STRING)}",
    ] {
        let css = format!(".box.hot{{color:red}}.box[data-edit]{{color:blue}}{dependent}");
        let old = page(&css, "");
        for new in [
            old.replace("class=box", "class='box hot'"),
            old.replace("class=box", "class=box data-edit=x"),
        ] {
            assert_eq!(both(&old, &new), (true, true), "{dependent}");
        }
    }
}

#[test]
fn changed_generated_attributes_and_dynamic_css_rebuild_synchronously() {
    for css in [
        ".box::after{content:attr(data-edit)}",
        ".box::after{content:ATTR(DATA-EDIT)}",
        r".box::after{content:a\74tr(\64 ata-edit)}",
        ".box{color:attr(data-edit type(<color>))}",
        ".box{--x:attr(data-edit type(*));color:var(--x)}",
    ] {
        let old = page(css, "").replace("class=box", "class=box data-edit=red");
        let new = old.replace("data-edit=red", "data-edit=blue");
        assert_eq!(both(&old, &new), (false, false), "{css}");
    }
}

#[test]
fn an_inline_generated_attribute_keeps_its_dependency() {
    for name in ["data-label", "data-edit"] {
        let inline = format!("class=box data-edit=red style=\"--x:attr({name});color:red\"");
        let old = page(".box[data-edit=blue]{color:blue}", "").replace("class=box", &inline);
        let new = old.replace("data-edit=red", "data-edit=blue");
        let retained = name == "data-label";
        assert_eq!(both(&old, &new), (retained, retained), "{name}");
    }
}

#[test]
fn typed_attribute_values_can_read_a_name_absent_from_the_stylesheet() {
    let old = page(".box{--x:attr(data-source type(*));color:var(--x)}", "").replace(
        "class=box",
        "class=box data-edit=red data-source='attr(data-edit type(<color>))'",
    );
    let new = old.replace("data-edit=red", "data-edit=blue");
    assert_eq!(both(&old, &new), (false, false));
}

#[test]
fn theme_attribute_dependencies_use_the_same_name_index() {
    let old = page(".box.hot{color:red}", "");
    let new = old.replace("class=box", "class='box hot'");
    for (theme, retained) in [
        ("p::after{content:attr(title)}", true),
        ("p::after{content:attr(class)}", false),
    ] {
        assert_eq!(super::super::update(&old, &new, theme), retained);
        assert_eq!(super::super::update(&new, &old, theme), retained);
    }
}
