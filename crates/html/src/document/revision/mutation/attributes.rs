//! Attribute writes and their ordered difference for one element.
use super::{Attribute, Handle, Mutation, QualName};

/// `setAttribute`/`removeAttribute` (DOM §4.9): значение на месте прежнего
/// атрибута, новый — в конец списка.
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

/// Записи, переводящие атрибуты `old` в список `new` с тем же порядком: общее
/// начало остаётся, хвост `old` снимается, хвост `new` ставится по порядку.
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
