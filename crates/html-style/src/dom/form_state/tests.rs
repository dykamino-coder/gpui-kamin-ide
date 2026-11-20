use super::*;

fn checked_ids(source: &str) -> Vec<String> {
    let dom = crate::dom::input::Admitted::parse(source, false)
        .unwrap()
        .dom;
    let state = FormState::default();
    let mut out = vec![];
    let mut stack = vec![dom.document];
    while let Some(node) = stack.pop() {
        if let Some((tag, attrs)) = element(&node)
            && state.checked(&node, tag, &attrs)
        {
            out.push(value(&attrs, "id").unwrap_or("?").to_string());
        }
        stack.extend(node.children.borrow().iter().rev().cloned());
    }
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
         <select size=3><option id=g></select>",
    );
    assert_eq!(ids, ["b", "d", "e", "f"]);
}
