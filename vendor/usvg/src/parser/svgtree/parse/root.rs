//! Keep the CSS root's computed value available before resolving inheritance.

use super::*;

pub(super) fn apply(parent: NodeId, tag: EId, start: usize, doc: &mut Document<'_>) {
    if tag != EId::Svg || parent != doc.root().id {
        return;
    }
    let Some(opacity) = doc.root_opacity else {
        return;
    };
    let value = roxmltree::StringStorage::new_owned(&opacity.get().to_string());
    if let Some(attr) = doc.attrs[start..].iter_mut().find(|a| a.name == AId::Opacity) {
        attr.value = value;
    } else {
        doc.append_attribute(AId::Opacity, value, false);
    }
}
