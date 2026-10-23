//! Каскад: явное наследование (`inherit`) ненаследуемых свойств вычисленным
//! значением родителя (CSS Cascade 4 §7.3.1), а не его заданными относительными
//! единицами. Выигравшее CSS-слово записывается в тот же слот, что и обычный
//! longhand (`explicit_inherit`); `apply_explicit_inheritance` копирует поля родителя по
//! реестру, `settle` делает это по всему дереву до прохода масштаба `zoom`.

use crate::document::state::ValueCx;
use crate::dom::Node;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

mod registry;
pub(crate) use registry::{Property, of_longhand};

/// Record winning CSS-wide keywords in the same slots as ordinary longhands.
pub(super) fn note(c: &mut Computed, key: &str, value: &str, vcx: ValueCx<'_>) -> bool {
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
            property.copy(&initial, c, vcx);
        }
        return true;
    }
    false
}

pub(super) fn apply_explicit_inheritance(c: &mut Computed, parent: &Computed, vcx: ValueCx<'_>) {
    for property in std::mem::take(&mut c.explicit_inherit) {
        property.copy(parent, c, vcx);
    }
}

/// Registry longhands with a cascaded `inherit` whose fields the registry
/// copies (multicol, gaps, grid, decoration, border-image ...).
fn registry_inherits(c: &Computed) -> impl Iterator<Item = crate::style::css::LonghandId> {
    c.inherit_longhands
        .iter()
        .filter(|id| !of_longhand(*id).is_empty())
}

/// Copy the registry fields of the parent for every registry property with a
/// cascaded `inherit`, and drop those marks: the values are the element's own.
fn settle_registry_inheritance(c: &mut Computed, parent: &Computed, vcx: ValueCx<'_>) {
    apply_explicit_inheritance(c, parent, vcx);
    for id in registry_inherits(c).collect::<Vec<_>>() {
        for property in of_longhand(id) {
            property.copy(parent, c, vcx);
        }
        c.inherit_longhands.set(id, false);
    }
}

/// CSS Cascade 4 section 7.3.1: inherit the computed value, never the parent's
/// specified relative units. The zoom pass subsequently scales absolute lengths
/// using this element's effective zoom, including nested inheritance chains;
/// layout still reads these non-inherited values from the cascaded style.
pub(crate) fn settle(nodes: &mut [Node], vcx: ValueCx<'_>) {
    fn pending(nodes: &[Node]) -> bool {
        nodes.iter().any(|n| match n {
            Node::Element(e) => {
                !e.style.explicit_inherit.is_empty()
                    || registry_inherits(&e.style).next().is_some()
                    || !matches!(e.style.svg_path_length, None | Some(Len::Px(_) | Len::Auto))
                    || pending(&e.children)
            }
            _ => false,
        })
    }
    fn apply_inheritance_to_descendants(nodes: &mut [Node], parent: &Computed, vcx: ValueCx<'_>) {
        for node in nodes {
            let Node::Element(e) = node else { continue };
            if !e.style.explicit_inherit.is_empty() || registry_inherits(&e.style).next().is_some()
            {
                settle_registry_inheritance(&mut e.style, parent, vcx);
            }
            let mut own = (*e.style).clone();
            let media = vcx.doc.media.get();
            own.resolve_viewport((media.width, media.height), vcx);
            let mut computed = super::inherit::inherit_unpainted(parent, &own, vcx);
            if let Some(mut length) = computed.svg_path_length {
                let font = computed.font_cx(vcx);
                length.resolve_line_height(font.line_px.unwrap_or(1.2 * font.font_px), vcx);
                if let Some(value) = length.to_px(None, font, vcx) {
                    computed.svg_path_length = Some(Len::Px(value.max(0.0)));
                }
                e.style.svg_path_length = computed.svg_path_length;
            }
            apply_inheritance_to_descendants(&mut e.children, &computed, vcx);
        }
    }
    if pending(nodes) {
        apply_inheritance_to_descendants(nodes, &Computed::default(), vcx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::values::value::Len;

    #[test]
    fn computed_inheritance_and_cascade_order() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut parent = Computed::default();
        parent.apply_one("column-width", "100px", vcx);
        let compute = super::super::compute::compute;
        let mut child = Computed::default();
        child.apply_one("column-width", "inherit", vcx);
        assert_eq!(
            compute(&parent, &child, vcx).column_width,
            Some(Len::Px(100.0))
        );
        child.apply_one("column-width", "inherit", vcx);
        child.apply_one("column-width", "auto", vcx);
        assert_eq!(compute(&parent, &child, vcx).column_width, None);
    }

    #[test]
    fn shorthand_inheritance_preserves_later_longhand() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut parent = Computed::default();
        parent.apply_one("gap", "10px 20px", vcx);
        let mut child = Computed::default();
        child.apply_one("gap", "inherit", vcx);
        child.apply_one("row-gap", "30px", vcx);
        let child = super::super::compute::compute(&parent, &child, vcx);
        assert_eq!(child.gap, Some((Some(Len::Px(30.0)), Some(Len::Px(20.0)))));
    }

    #[test]
    fn initial_removes_both_previous_value_and_inheritance() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut child = Computed::default();
        child.apply_one("column-width", "60px", vcx);
        child.apply_one("column-width", "inherit", vcx);
        child.apply_one("column-width", "initial", vcx);
        assert_eq!(child.column_width, None);
        assert!(child.explicit_inherit.is_empty());
    }

    #[test]
    fn direct_inheritance_uses_computed_parent_units() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut specified = Computed::default();
        specified.apply_one("font-size", "20px", vcx);
        specified.apply_one("column-width", "5em", vcx);
        let parent =
            super::super::inherit::inherit_unpainted(&Computed::default(), &specified, vcx);
        let mut child = Computed::default();
        child.apply_one("font-size", "10px", vcx);
        child.apply_one("column-width", "inherit", vcx);
        let computed = super::super::inherit::inherit_unpainted(&parent, &child, vcx);
        assert_eq!(computed.column_width, Some(Len::Px(100.0)));
        assert_eq!(computed.font_size, Some(Len::Px(10.0)));
    }
}
