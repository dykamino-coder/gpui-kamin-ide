//! Stacking-context boundary elements open and drain paint collectors.

use super::*;

/// Пара меток собирателя без своей коробки: открывающая ставится первым
/// ребёнком корня документа, закрывающая — последним; между ними `PaintLast`
/// уходит в собиратель, а на закрывающей дорисовывается по ключам.
pub struct PaintCollect {
    open: bool,
    state: Rc<Cell<Option<(usize, bool)>>>,
}

impl PaintCollect {
    /// Пара меток: открывающая и закрывающая.
    pub fn pair() -> (PaintCollect, PaintCollect) {
        let state: Rc<Cell<Option<(usize, bool)>>> = Rc::default();
        (
            PaintCollect {
                open: true,
                state: state.clone(),
            },
            PaintCollect { open: false, state },
        )
    }
}

impl Element for PaintCollect {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        // Внепоточная коробка нулевого размера: соседей не двигает.
        let style = Style {
            position: crate::Position::Absolute,
            ..Default::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.open {
            self.state.set(Some(open_frame()));
        } else if let Some((depth, prev)) = self.state.take() {
            close_frame(depth, prev, window, cx);
        }
    }
}

impl IntoElement for PaintCollect {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
