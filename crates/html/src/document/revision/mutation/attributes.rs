//! Attribute writes and their ordered difference for one element.
use super::{Attribute, Handle, Mutation, QualName};

/// Set or remove an attribute in place. A new attribute goes at the end.
pub(super) fn set(list: &mut Vec<Attribute>, name: &QualName, value: Option<&str>) {
    let at = list.iter().position(|a| a.name == *name);
    match (at, value) {
        (Some(at), Some(value)) => list[at].value = value.into(),
        (None, Some(value)) => list.push(Attribute {
            name: name.clone(),
            value: value.into(),
        }),
        (Some(at), None) => {
            list.remove(at);
        }
        (None, None) => {}
    }
}

/// Preserve the common prefix, remove the old tail, and append the new tail
/// in source order.
pub(in crate::document::revision) fn attribute_records(
    element: &Handle,
    old: &[Attribute],
    new: &[Attribute],
) -> Vec<Mutation> {
    let common = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let record = |name: &QualName, value: Option<String>| Mutation::Attribute {
        element: element.clone(),
        name: name.clone(),
        value,
    };
    old[common..]
        .iter()
        .map(|a| record(&a.name, None))
        .chain(
            new[common..]
                .iter()
                .map(|a| record(&a.name, Some(a.value.to_string()))),
        )
        .collect()
}
