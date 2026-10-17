//! Select reset regressions on trees with grouped, disabled and nested options.

use crate::{append, Handle, Node, NodeData};
use markup5ever::{Attribute, QualName};
use std::{cell::RefCell, rc::Rc};

pub(super) fn element(name: &str, attrs: &[(&str, &str)], children: Vec<Handle>) -> Handle {
    let node = Node::new(NodeData::Element {
        name: QualName::new(None, "http://www.w3.org/1999/xhtml".into(), name.into()),
        attrs: RefCell::new(
            attrs
                .iter()
                .map(|(key, value)| Attribute {
                    name: QualName::new(None, "".into(), (*key).into()),
                    value: (*value).into(),
                })
                .collect(),
        ),
        template_contents: RefCell::new(None),
        mathml_annotation_xml_integration_point: false,
    });
    for child in children {
        append(&node, child);
    }
    node
}

fn option(attrs: &[(&str, &str)]) -> Handle {
    element("option", attrs, vec![])
}

fn chosen(select: &Handle, option: &Handle) {
    assert!(Rc::ptr_eq(&select.selected_option().unwrap(), option));
    assert!(option.is_selected_option());
}

#[test]
fn last_selected_uses_tree_order_across_optgroups() {
    let grouped = option(&[("selected", "")]);
    let direct = option(&[("selected", "")]);
    let select = element(
        "select",
        &[],
        vec![
            element("optgroup", &[], vec![grouped.clone()]),
            direct.clone(),
        ],
    );
    chosen(&select, &direct);
    assert!(!grouped.is_selected_option());
}

#[test]
fn fallback_skips_disabled_options_and_optgroups() {
    let disabled = option(&[("disabled", "")]);
    let grouped = option(&[]);
    let enabled = option(&[]);
    let select = element(
        "select",
        &[],
        vec![
            disabled.clone(),
            element("optgroup", &[("disabled", "")], vec![grouped.clone()]),
            enabled.clone(),
        ],
    );
    chosen(&select, &enabled);
    assert!(!disabled.is_selected_option());
    assert!(!grouped.is_selected_option());
}

#[test]
fn explicit_disabled_selection_is_preserved() {
    let selected = option(&[("selected", ""), ("disabled", "")]);
    let select = element("select", &[], vec![option(&[]), selected.clone()]);
    chosen(&select, &selected);
}

#[test]
fn no_enabled_option_means_no_fallback() {
    let select = element("select", &[], vec![option(&[("disabled", "")])]);
    assert!(select.selected_option().is_none());
}

#[test]
fn fallback_requires_display_size_one() {
    for size in ["", "bad", "1", " +0001tail", "-1"] {
        let first = option(&[]);
        let select = element("select", &[("size", size)], vec![first.clone()]);
        chosen(&select, &first);
    }
    for size in ["0", "2", "10", "99999999999999999999999999999999999"] {
        let select = element("select", &[("size", size)], vec![option(&[])]);
        assert!(select.selected_option().is_none(), "size={size}");
    }
}

#[test]
fn multiple_preserves_each_explicit_selection_and_has_no_fallback() {
    let first = option(&[("selected", "")]);
    let second = option(&[("selected", "")]);
    let unselected = option(&[]);
    let select = element(
        "select",
        &[("multiple", "")],
        vec![first.clone(), second.clone(), unselected.clone()],
    );
    chosen(&select, &first);
    assert!(second.is_selected_option());
    assert!(!unselected.is_selected_option());
    assert!(element("select", &[("multiple", "")], vec![option(&[])])
        .selected_option()
        .is_none());
}

#[test]
fn excluded_subtrees_do_not_contribute_options() {
    let direct = option(&[]);
    let select = element(
        "select",
        &[],
        vec![
            direct.clone(),
            element("select", &[], vec![option(&[("selected", "")])]),
            element("datalist", &[], vec![option(&[("selected", "")])]),
            element("hr", &[], vec![option(&[("selected", "")])]),
            element(
                "optgroup",
                &[],
                vec![element("optgroup", &[], vec![option(&[("selected", "")])])],
            ),
            element(
                "option",
                &[("disabled", "")],
                vec![option(&[("selected", "")])],
            ),
        ],
    );
    chosen(&select, &direct);
}
