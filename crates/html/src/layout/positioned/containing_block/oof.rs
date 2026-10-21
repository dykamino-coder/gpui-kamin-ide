//! Носители внепоточных детей слоя CB, без новой коробки в дереве GPUI.

use super::SpotCell;
use gpui::AnyElement;

pub(super) struct OofKid {
    spot: SpotCell,
    element: AnyElement,
}

#[derive(Default)]
pub(super) struct OofHost(Vec<OofKid>);

impl OofHost {
    pub(super) fn push(&mut self, spot: SpotCell, element: AnyElement) {
        self.0.push(OofKid { spot, element });
    }

    pub(super) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(super) fn place(self, place: fn(SpotCell, AnyElement) -> AnyElement) -> Vec<AnyElement> {
        self.0
            .into_iter()
            .map(|kid| place(kid.spot, kid.element))
            .collect()
    }
}
