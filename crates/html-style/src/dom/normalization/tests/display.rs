//! The production display pass consumes raw contexts and preserves old frames.
use super::*;
use crate::dom::display_inheritance::DisplayInheritance;
use crate::style::computed::{Display, inh};

fn inherited() -> Node {
    let mut element = Element::synthetic("b", Default::default(), Default::default(), vec![]);
    element.style.cascaded_mut().reset_cascade.inherit_bits |= inh::DISPLAY;
    Node::Element(element)
}

#[test]
fn implicit_html_roles_and_parent_changes_match_a_fresh_display_pass() {
    let input: Children = vec![node("tbody", vec![inherited()])].into();
    let mut pass = Pass::new(DisplayInheritance);
    let context = (None, None, None, None, None);
    let old = publish(&mut pass, &input, context, &Admission::default());
    let child = element(&element(&old[0]).children[0]);
    assert_eq!(
        child.style.cascaded().reset_box_layout.display,
        Some(Display::TableRowGroup)
    );
    assert_eq!(child.style.cascaded().reset_context.row_group_kind, Some(1));
    assert_eq!(
        child.style.cascaded().reset_cascade.inherit_bits & inh::DISPLAY,
        0
    );
    assert_ne!(
        element(&element(&input[0]).children[0])
            .style
            .cascaded()
            .reset_cascade
            .inherit_bits
            & inh::DISPLAY,
        0
    );
    let mut changed = input.clone();
    element_mut(&mut changed[0]).tag = "tfoot".into();
    let actual = publish(&mut pass, &changed, context, &Admission::default());
    assert_eq!(
        element(&element(&actual[0]).children[0])
            .style
            .cascaded()
            .reset_context
            .row_group_kind,
        Some(2)
    );
    assert_eq!(child.style.cascaded().reset_context.row_group_kind, Some(1));
    let fresh = publish(
        &mut Pass::new(DisplayInheritance),
        &changed,
        context,
        &Admission::default(),
    );
    assert!(same(&actual, &fresh));
}
