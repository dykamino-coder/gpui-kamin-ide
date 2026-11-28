//! Actual CSS Text inputs of inline descendants, independent of paint and line width.

use crate::dom::BoxId;
use crate::style::computed::Computed;
use crate::text::inline::normalization::Inputs;
use std::{collections::HashMap, rc::Rc};

pub(super) type Styles = HashMap<BoxId, (Rc<Computed>, bool)>;

pub(super) fn matches(dependencies: &[(BoxId, Inputs)], styles: &Styles) -> bool {
    dependencies.iter().all(|(id, input)| {
        styles.get(id).is_some_and(|(style, ready)| {
            *ready && input.matches(style, style.reset_text_box.ruby_suppress_breaks)
        })
    })
}

pub(super) fn input<'a>(
    owner: BoxId,
    id: BoxId,
    root: &'a Computed,
    styles: &'a Styles,
    dependencies: &mut Vec<(BoxId, Inputs)>,
) -> Option<&'a Computed> {
    if id == owner {
        return Some(root);
    }
    let (style, ready) = styles.get(&id)?;
    if !ready {
        return None;
    }
    if dependencies.last().is_none_or(|(last, _)| *last != id) {
        dependencies.push((
            id,
            Inputs::of(style, style.reset_text_box.ruby_suppress_breaks),
        ));
    }
    Some(style)
}
