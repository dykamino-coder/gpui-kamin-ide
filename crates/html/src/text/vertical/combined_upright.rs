//! Combined upright for vertical; split out to keep the owning module within 250 lines.

use super::combined_geometry;
use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window, px,
};

/// Строка вертикального письма: `writing-mode: vertical-rl` и `vertical-lr`.
///
/// Поворота мало: у повёрнутого текста меняются местами ширина и высота, и
/// раскладка обязана считать их поменянными — иначе строка занимает место как
/// горизонтальная и наезжает на соседей. Поэтому это свой элемент: он
/// измеряет текст сам, отдаёт раскладке перевёрнутый размер, а на отрисовке
/// разворачивает содержимое на четверть оборота по часовой стрелке.
/// `text-combine-upright`: составной знак в вертикальной строке.
///
/// Абзац вертикального письма рисуется ПОВОРОТОМ на четверть по часовой;
/// сжатый кусок обязан остаться стоячим — он контр-поворачивается вокруг
/// СВОЕГО ЦЕНТРА (квадрат кегля переходит в себя) и ужимается по строчной
/// оси в один кегль (css-writing-modes-3 §9.1).
pub struct CombinedUpright {
    pub(crate) child: Option<AnyElement>,
    /// Кегль — сторона квадрата, который кусок занимает в строке.
    pub(crate) em: f32,
    /// Сжимать ли содержимое в кегль: у `text-combine-upright` — да, у
    /// стоячего `inline-block` с горизонтальным письмом — нет, он просто
    /// переполняет свой квадрат.
    pub(crate) compress: bool,
    pub(crate) natural: gpui::Size<Pixels>,
}

impl CombinedUpright {
    pub fn new(child: AnyElement, em: f32) -> Self {
        CombinedUpright {
            child: Some(child),
            em,
            compress: true,
            natural: gpui::Size::default(),
        }
    }

    /// Стоячая коробка без сжатия (см. поле `compress`).
    pub fn upright_box(child: AnyElement, em: f32) -> Self {
        CombinedUpright {
            child: Some(child),
            em,
            compress: false,
            natural: gpui::Size::default(),
        }
    }
}

impl Element for CombinedUpright {
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
        let space = gpui::size(
            gpui::AvailableSpace::MaxContent,
            gpui::AvailableSpace::MaxContent,
        );
        self.natural = self
            .child
            .as_mut()
            .unwrap()
            .layout_as_root(space, window, cx);
        let mut style = gpui::Style::default();
        let side = gpui::Length::Definite(gpui::DefiniteLength::Absolute(
            gpui::AbsoluteLength::Pixels(px(self.em)),
        ));
        style.size.width = side;
        style.size.height = side;
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let child = self.child.as_mut().unwrap();
        child.prepaint_at(bounds.origin, window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let matrix = combined_geometry::transform(
            bounds,
            self.natural,
            self.em,
            self.compress,
            window.scale_factor(),
        );
        let child = self.child.as_mut().unwrap();
        window.with_transformation(matrix, |window| child.paint(window, cx));
    }
}

impl IntoElement for CombinedUpright {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}
