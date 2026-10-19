//! Combined upright for vertical; split out to keep the owning module within 250 lines.

use super::combined_geometry;
use crate::hooks::Wrap;
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Window, px};

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
    pub(crate) child: AnyElement,
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
            child,
            em,
            compress: true,
            natural: gpui::Size::default(),
        }
    }

    /// Стоячая коробка без сжатия (см. поле `compress`).
    pub fn upright_box(child: AnyElement, em: f32) -> Self {
        CombinedUpright {
            child,
            em,
            compress: false,
            natural: gpui::Size::default(),
        }
    }
}

impl Wrap for CombinedUpright {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        let space = gpui::size(
            gpui::AvailableSpace::MaxContent,
            gpui::AvailableSpace::MaxContent,
        );
        self.natural = self.child.layout_as_root(space, window, cx);
        let mut style = gpui::Style::default();
        let side = gpui::Length::Definite(gpui::DefiniteLength::Absolute(
            gpui::AbsoluteLength::Pixels(px(self.em)),
        ));
        style.size.width = side;
        style.size.height = side;
        window.request_layout(style, [], cx)
    }

    fn prepaint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let child = &mut self.child;
        child.prepaint_at(bounds.origin, window, cx);
    }

    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let matrix = combined_geometry::transform(
            bounds,
            self.natural,
            self.em,
            self.compress,
            window.scale_factor(),
        );
        let matrix = combined_geometry::in_local_space(matrix, window.current_transformation());
        let child = &mut self.child;
        window.with_transformation(matrix, |window| child.paint(window, cx));
    }
}
