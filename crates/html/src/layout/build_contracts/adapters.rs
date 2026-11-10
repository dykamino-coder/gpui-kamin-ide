//! Дополнительные запросы построителя: SVG-содержимое, sizing, native-значения и outline-фаза.

use super::{ChildBuild, GridPlacement};
use crate::dom::Element;
use crate::layout::cx::LayoutCx;
use crate::style::computed::{Align, BoxStyle};
use crate::style::values::value::Len;
use crate::values::ValueCx;
use gpui::AnyElement;

impl ChildBuild {
    pub(crate) fn for_geometry(
        mut self,
        paragraph: fn(&[crate::dom::Node], &BoxStyle, &crate::RenderOpts) -> AnyElement,
        styled_box: fn(
            &Element,
            &BoxStyle,
            &LayoutCx,
            Option<crate::layout::block::sizing::BoxDimensions>,
        ) -> gpui::Div,
    ) -> Self {
        self.flow = self.flow.with_paragraph(paragraph);
        self.boxes = self.boxes.with_styled_box(styled_box);
        self
    }
    /// Native form control with its merged style; `None` for other tags.
    pub(crate) fn control(
        &self,
        e: &Element,
        style: &BoxStyle,
        opts: &crate::layout::options::RenderOpts,
    ) -> Option<AnyElement> {
        (self.control)(e, style, opts)
    }

    /// Рисует formatting context в текущем сборщике outline либо открывает собственную фазу.
    pub(crate) fn outline_flow(
        &self,
        owner: &std::rc::Rc<crate::layout::cx::deferred::Deferred>,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
        body: super::effects::PaintBody<'_>,
    ) {
        self.effects.outline_flow(owner, window, cx, body)
    }

    /// Содержимое встроенного SVG, включая HTML foreign objects.
    pub(crate) fn svg(
        &self,
        e: &Element,
        opts: &crate::layout::options::RenderOpts,
    ) -> Option<AnyElement> {
        (self.svg)(e, opts)
    }

    /// Предпочтительные размеры содержимого с учётом replaced/native-коробки и наследования.
    pub(crate) fn content_for_element(
        &self,
        child: AnyElement,
        e: &Element,
        parent: &BoxStyle,
        placement: GridPlacement,
        vcx: ValueCx<'_>,
    ) -> AnyElement {
        self.boxes
            .content_for_element(child, e, parent, placement, vcx)
    }

    /// Вычисленная CSS-длина в представлении native layout adapter.
    pub(crate) fn length(&self, length: Len, vcx: ValueCx<'_>) -> gpui::DefiniteLength {
        self.boxes.length(length, vcx)
    }

    /// Вычисленное self-alignment в представлении native layout adapter.
    pub(crate) fn alignment(&self, align: Align, last: bool) -> gpui::AlignItems {
        self.boxes.alignment(align, last)
    }
}
