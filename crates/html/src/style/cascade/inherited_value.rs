//! Explicit inheritance uses computed longhands, before effective zoom is applied.

use crate::dom::Node;
use crate::style::computed::Computed;

mod registry;
pub(crate) use registry::Property;

/// Record winning CSS-wide keywords in the same slots as ordinary longhands.
pub(super) fn note(c: &mut Computed, key: &str, value: &str) -> bool {
    let properties = registry::properties(key);
    if properties.is_empty() {
        return false;
    }
    c.explicit_inherit.retain(|p| !properties.contains(p));
    if value.eq_ignore_ascii_case("inherit") {
        c.explicit_inherit.extend_from_slice(properties);
        return true;
    }
    if matches!(value, "initial" | "unset" | "revert" | "revert-layer") {
        let initial = Computed::default();
        for property in properties {
            property.copy(&initial, c);
        }
        return true;
    }
    false
}

pub(super) fn resolve(c: &mut Computed, parent: &Computed) {
    for property in std::mem::take(&mut c.explicit_inherit) {
        property.copy(parent, c);
    }
}

/// CSS Cascade 4 section 7.3.1: inherit the computed value, never the parent's
/// specified relative units. The zoom pass subsequently scales absolute lengths
/// using this element's effective zoom, including nested inheritance chains.
pub(crate) fn settle(nodes: &mut [Node]) {
    fn pending(nodes: &[Node]) -> bool {
        nodes.iter().any(|n| match n {
            Node::Element(e) => !e.style.explicit_inherit.is_empty() || pending(&e.children),
            _ => false,
        })
    }
    fn walk(nodes: &mut [Node], parent: &Computed) {
        for node in nodes {
            let Node::Element(e) = node else { continue };
            resolve(&mut e.style, parent);
            let computed = super::inherit::inherit_unpainted(parent, &e.style);
            walk(&mut e.children, &computed);
        }
    }
    if pending(nodes) {
        walk(nodes, &Computed::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::values::value::Len;

    #[test]
    fn computed_inheritance_and_cascade_order() {
        let mut parent = Computed::default();
        parent.apply_one("column-width", "100px");
        let mut child = Computed::default();
        child.apply_one("column-width", "inherit");
        resolve(&mut child, &parent);
        assert_eq!(child.column_width, Some(Len::Px(100.0)));
        child.apply_one("column-width", "inherit");
        child.apply_one("column-width", "auto");
        resolve(&mut child, &parent);
        assert_eq!(child.column_width, None);
    }

    #[test]
    fn shorthand_inheritance_preserves_later_longhand() {
        let mut parent = Computed::default();
        parent.apply_one("gap", "10px 20px");
        let mut child = Computed::default();
        child.apply_one("gap", "inherit");
        child.apply_one("row-gap", "30px");
        resolve(&mut child, &parent);
        assert_eq!(child.gap, Some((Some(Len::Px(30.0)), Some(Len::Px(20.0)))));
    }

    #[test]
    fn initial_removes_both_previous_value_and_inheritance() {
        let mut child = Computed::default();
        child.apply_one("column-width", "60px");
        child.apply_one("column-width", "inherit");
        child.apply_one("column-width", "initial");
        assert_eq!(child.column_width, None);
        assert!(child.explicit_inherit.is_empty());
    }

    #[test]
    fn direct_inheritance_uses_computed_parent_units() {
        let mut specified = Computed::default();
        specified.apply_one("font-size", "20px");
        specified.apply_one("column-width", "5em");
        let parent = super::super::inherit::inherit_unpainted(&Computed::default(), &specified);
        let mut child = Computed::default();
        child.apply_one("font-size", "10px");
        child.apply_one("column-width", "inherit");
        let computed = super::super::inherit::inherit_unpainted(&parent, &child);
        assert_eq!(computed.column_width, Some(Len::Px(100.0)));
        assert_eq!(computed.font_size, Some(Len::Px(10.0)));
    }
}
