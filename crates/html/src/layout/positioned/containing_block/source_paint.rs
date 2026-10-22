//! Разделение позиционированной коробки на две половины: раскладку у содержащего блока и
//! отрисовку на месте в дереве. CSS 2.1 Appendix E красит позиционированные в порядке
//! разметки независимо от содержащего блока, поэтому `SourcePaint` без заместителя
//! запрашивает раскладку у CB, а с заместителем делает `prepaint`/`paint` из исходного места.

use super::{LatePlace, SpotCell};
use crate::hooks::{Hooks, IntoHooked};
use gpui::{
    AnyElement, App, Bounds, IntoElement, LayoutId, Pixels, Styled, Window, div, point, px,
};
use std::{cell::RefCell, rc::Rc};

/// CSS 2 Appendix E orders painting independently of the containing block.
/// Only the layout half belongs to the CB; the source half owns prepaint/paint.
pub(crate) fn split(spot: SpotCell, child: AnyElement) -> (AnyElement, AnyElement) {
    let child = Rc::new(RefCell::new(LatePlace { child, spot }.hooked()));
    let layout = SourcePaint {
        child: child.clone(),
        placeholder: None,
    };
    let paint = SourcePaint {
        child,
        placeholder: Some(div().absolute().w_0().h_0().into_any_element()),
    };
    (layout.hooked(), paint.hooked())
}

struct SourcePaint {
    child: Rc<RefCell<AnyElement>>,
    placeholder: Option<AnyElement>,
}

impl Hooks for SourcePaint {
    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let layout = match self.placeholder.as_mut() {
            Some(placeholder) => placeholder.request_layout(window, cx),
            None => self.child.borrow_mut().request_layout(window, cx),
        };
        (layout, ())
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) {
        if self.placeholder.is_some() {
            self.child
                .borrow_mut()
                .prepaint_at(point(px(0.0), px(0.0)), window, cx);
        }
    }

    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.placeholder.is_some() {
            gpui::paint_reopen(|| self.child.borrow_mut().paint(window, cx));
        }
    }
}

/// Корень строится у CB на prepaint, а красится в исходном месте дерева.
#[derive(Clone, Default)]
pub(crate) struct DeferredPaint(Rc<RefCell<Option<DeferredChild>>>);

type DeferredChild = (AnyElement, gpui::ContentMask<Pixels>);

impl DeferredPaint {
    pub(crate) fn publish(&self, child: AnyElement, mask: gpui::ContentMask<Pixels>) {
        *self.0.borrow_mut() = Some((child, mask));
    }

    pub(crate) fn source(&self) -> AnyElement {
        self.clone().hooked()
    }
}

impl Hooks for DeferredPaint {
    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let style = gpui::Style {
            position: gpui::Position::Absolute,
            ..Default::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, _: &mut (), _: &mut Window, _: &mut App) {}

    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some((child, mask)) = self.0.borrow_mut().as_mut() {
            // Маска исходного слоя дополняет обрезку фрагмента, заданную при paint.
            window.with_content_mask(Some(*mask), |window| {
                gpui::paint_reopen(|| child.paint(window, cx));
            });
        }
    }
}
