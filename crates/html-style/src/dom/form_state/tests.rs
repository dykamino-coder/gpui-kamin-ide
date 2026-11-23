//! Checkedness and selectedness derived from content attributes.

use super::*;

fn checked_ids(source: &str) -> Vec<String> {
    let dom = crate::dom::input::Admitted::parse(source, false)
        .unwrap()
        .dom;
    let state = FormState::default();
    let mut out = vec![];
    // Parent links are weak: the document must outlive the walk, or a popped
    // ancestor is freed and a radio button no longer finds its tree root.
    let document = dom.document;
    let mut stack = vec![document.clone()];
    while let Some(node) = stack.pop() {
        if let Some((tag, attrs)) = element(&node)
            && state.checked(&node, tag, &attrs)
        {
            out.push(value(&attrs, "id").unwrap_or("?").to_string());
        }
        stack.extend(node.children.borrow().iter().rev().cloned());
    }
    drop(document);
    out
}

#[test]
fn checkbox_and_unnamed_radio_follow_the_attribute() {
    let ids = checked_ids(
        "<input id=a type=checkbox checked><input id=b type=CHECKBOX>\
         <input id=c type=radio checked><input id=d type=radio checked>\
         <input id=e checked><div id=f checked></div>",
    );
    assert_eq!(ids, ["a", "c", "d"]);
}

#[test]
fn radio_group_keeps_its_last_checked_button_per_form_owner() {
    let ids = checked_ids(
        "<input id=a type=radio name=g checked><input id=b type=radio name=g checked>\
         <input id=c type=radio name=h checked>\
         <form><input id=d type=radio name=g checked></form>\
         <input id=e type=radio name=G checked>",
    );
    assert_eq!(ids, ["b", "c", "d", "e"]);
}

#[test]
fn explicit_and_ancestor_form_owners_share_one_radio_group() {
    let ids = checked_ids(
        "<form id=f><input id=a type=radio name=g checked></form>\
         <input id=b type=radio name=g checked form=f>\
         <input id=c type=radio name=g checked form=later>\
         <form id=later><input id=d type=radio name=g checked></form>",
    );
    assert_eq!(ids, ["b", "d"]);
}

#[test]
fn missing_or_non_form_targets_leave_radios_without_a_form_owner() {
    let ids = checked_ids(
        "<input id=a type=radio name=g checked form=missing>\
         <svg id=duplicate></svg><form id=duplicate>\
         <input id=b type=radio name=g checked form=duplicate></form>\
         <input id=c type=radio name=g checked>",
    );
    assert_eq!(ids, ["c"]);
}

#[test]
fn an_empty_form_attribute_cannot_associate_with_an_empty_id() {
    assert_eq!(
        checked_ids(
            "<form id=''><input id=a type=radio name=g checked form=''></form>\
         <input id=b type=radio name=g checked>"
        ),
        ["b"]
    );
}

#[test]
fn checked_pseudo_class_matches_subjects_siblings_and_ancestors() {
    let parsed = crate::dom::parse(
        "<style>b{width:1px} :checked + b{width:7px} :checked ~ i b{width:9px}\
         label:has(:checked) u{width:5px}</style>\
         <input type=radio name=g checked><b id=x></b>\
         <input type=radio name=g checked><b id=y></b><i><b id=z></b></i>\
         <label><input type=checkbox checked><u id=w></u></label>",
        "",
    );
    let width = |id: &str| {
        crate::dom::test_support::find(&parsed.nodes, &|e| e.attr("id") == Some(id))
            .unwrap()
            .style
            .used()
            .reset_box_layout
            .width
            .clone()
    };
    use crate::style::values::value::Len;
    assert_eq!(width("x"), Some(Len::Px(1.0)));
    assert_eq!(width("y"), Some(Len::Px(7.0)));
    assert_eq!(width("z"), Some(Len::Px(9.0)));
    assert_eq!(width("w"), Some(Len::Px(5.0)));
}

#[test]
fn single_select_has_one_selected_option() {
    let ids = checked_ids(
        "<select><option id=a selected><option id=b selected></select>\
         <select><option id=c disabled><optgroup><option id=d></optgroup></select>\
         <select multiple><option id=e selected><option id=f selected></select>\
         <select size=3><option id=g></select>\
         <select size=3><option id=h selected><option id=i selected></select>",
    );
    assert_eq!(ids, ["b", "d", "e", "f", "i"]);
}

#[test]
fn select_size_uses_html_non_negative_integer_parsing() {
    let ids = checked_ids(
        "<select size='+3rows'><option id=a></select>\
         <select size='+1row'><option id=b></select>\
         <select size='-3'><option id=c></select>",
    );
    assert_eq!(ids, ["b", "c"]);
}
