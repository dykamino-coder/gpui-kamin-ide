//! Selected options, resolved once per select and style-tree parse.

use super::*;

/// Selectedness of an `option` (HTML В§4.10.10 "selectedness setting
/// algorithm"); outside a `select` it is the `selected` attribute.
impl FormState {
    pub(super) fn selected(&self, option: &Handle, attrs: &[(String, String)]) -> bool {
        let own = value(attrs, "selected").is_some();
        let Some(select) = owner_select(option) else {
            return own;
        };
        let Some((_, select_attrs)) = element(&select) else {
            return own;
        };
        let multiple = value(&select_attrs, "multiple").is_some();
        let size = value(&select_attrs, "size")
            .and_then(super::super::presentational_hr::non_negative_integer)
            .filter(|&s| s > 0)
            .unwrap_or(if multiple { 4 } else { 1 });
        if multiple {
            return own;
        }
        let mut selections = self.selections.borrow_mut();
        let chosen = selections.entry(HandleKey::of(&select)).or_insert_with(|| {
            // Resolve the list once per select in this parse, rather than scanning
            // all siblings for every option tested by a selector.
            let options = list_of_options(&select);
            options
                .iter()
                .rev()
                .find(|(_, attrs, _)| value(attrs, "selected").is_some())
                .or_else(|| {
                    options
                        .iter()
                        .find(|(_, _, disabled)| size == 1 && !disabled)
                })
                .map(|(handle, _, _)| HandleKey::of(handle))
        });
        chosen
            .as_ref()
            .is_some_and(|key| key == &HandleKey::of(option))
    }
}

/// The `select` whose list of options holds `option`: its parent, or the
/// parent of its `optgroup` parent.
fn owner_select(option: &Handle) -> Option<Handle> {
    let up = parent(option)?;
    let (tag, _) = element(&up)?;
    match tag {
        "select" => Some(up),
        "optgroup" => parent(&up).filter(|s| element(s).is_some_and(|(t, _)| t == "select")),
        _ => None,
    }
}

type Listed = (Handle, Vec<(String, String)>, bool);

/// The list of options of `select` in tree order, each with its attributes
/// and whether it is disabled (its own attribute or its `optgroup`'s).
fn list_of_options(select: &Handle) -> Vec<Listed> {
    let mut out = Vec::new();
    let mut push = |node: &Handle, group_disabled: bool| {
        if let Some(("option", attrs)) = element(node) {
            let disabled = group_disabled || value(&attrs, "disabled").is_some();
            out.push((node.clone(), attrs, disabled));
        }
    };
    for child in select.children.borrow().iter() {
        match element(child) {
            Some(("optgroup", attrs)) => {
                let disabled = value(&attrs, "disabled").is_some();
                for grand in child.children.borrow().iter() {
                    push(grand, disabled);
                }
            }
            Some(_) => push(child, false),
            None => {}
        }
    }
    out
}
