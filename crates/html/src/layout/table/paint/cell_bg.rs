//! Пробы фона ячеек, рамки сетки и кромок: что красят полосы и художник кромок.

use super::{CellEdges, EdgeCell};
use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Styled, Window,
};

/// Фоны ячеек сросшейся модели: прямоугольник и цвет, снятые пробой ячейки
/// на ПОДГОТОВКЕ кадра; красит их `CellBgPainter` — слой, лежащий в сетке
/// ПЕРЕД кромками и ячейками. Так фон ячейки оказывается под кромками, а
/// содержимое ячейки — над ними, как у Blink: сросшиеся кромки идут в фазе
/// `kDescendantBlockBackgroundsOnly` (`box_fragment_painter.cc:952-957`,
/// «Collapsed borders paint *after* children have painted their
/// backgrounds»), а строчное, плавающее и позиционированное содержимое
/// ячеек — в более поздних фазах, то есть поверх кромок.
pub type CellBgs = std::rc::Rc<std::cell::RefCell<Vec<(Bounds<Pixels>, gpui::Hsla)>>>;

/// Проба фона ячейки: холст во всю коробку ячейки записывает её рамку и
/// цвет на подготовке кадра (та же механика, что `edge_probe`).
pub fn cell_bg_probe(bgs: CellBgs, colour: gpui::Hsla) -> AnyElement {
    gpui::canvas(
        move |bounds: Bounds<Pixels>, _, _| {
            bgs.borrow_mut().push((bounds, colour));
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}

/// Слой фонов ячеек сросшейся таблицы (см. `CellBgs`).
pub struct CellBgPainter {
    pub(crate) bgs: CellBgs,
}

impl CellBgPainter {
    pub fn new(bgs: CellBgs) -> Self {
        CellBgPainter { bgs }
    }
}

impl Element for CellBgPainter {
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
        let mut style = gpui::Style::default();
        style.position = gpui::Position::Absolute;
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
        _cx: &mut App,
    ) {
        // Пробы пишут на подготовке, вся подготовка кадра идёт до отрисовки —
        // здесь прямоугольники ЭТОГО ЖЕ кадра.
        let bgs = std::mem::take(&mut *self.bgs.borrow_mut());
        for (bounds, colour) in bgs {
            window.paint_quad(gpui::fill(bounds, colour));
        }
    }
}

impl IntoElement for CellBgPainter {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Проба кромок: как проба фона, пишет в PREPAINT границы и рамки ячейки.
/// Метка «коробка СЕТКИ»: проба ничего не рисует, а сообщает слою кромок,
/// где кончаются дорожки. Граница сетки есть ВСЕГДА, даже когда рисующей
/// кромки у таблицы нет, и смешивать эти два понятия нельзя (замеры «крайние
/// линии только с `source == 0`» и «нулевая проба таблицы» — обе давали
/// +21/-25).
pub const GRID_BOX: u8 = u8::MAX;

/// Проба ГРАНИЦЫ СЕТКИ: координаты те же, что у пробы кромок таблицы
/// (`inset` — ширины её рамочного места), но без самих кромок.
pub fn grid_probe(edges: CellEdges, inset: [f32; 4]) -> AnyElement {
    edge_probe(
        edges,
        [0.0; 4],
        [crate::style::values::value::Color::default(); 4],
        [0; 4],
        GRID_BOX,
        0,
        inset,
    )
}

pub fn edge_probe(
    edges: CellEdges,
    widths: [f32; 4],
    colors: [crate::style::values::value::Color; 4],
    styles: [u8; 4],
    source: u8,
    doc_ix: u32,
    inset: [f32; 4],
) -> AnyElement {
    // Exact (unrounded) layout bounds: each collapsed border band is snapped
    // once, from the grid line it is centred on (CSS 2.1 §17.6.2). Bands
    // built from independently rounded cell edges left a device-pixel gap
    // between two bands whose exact edges coincide.
    gpui::canvas_with_unrounded_bounds(
        move |bounds: Bounds<Pixels>, _, _| {
            // Вжим границ внутрь: линии рамки самой таблицы лежат на
            // ВНУТРЕННИХ краях её рамочного места.
            let bounds = Bounds {
                origin: gpui::point(
                    bounds.origin.x + gpui::px(inset[3]),
                    bounds.origin.y + gpui::px(inset[0]),
                ),
                size: gpui::size(
                    bounds.size.width - gpui::px(inset[1] + inset[3]),
                    bounds.size.height - gpui::px(inset[0] + inset[2]),
                ),
            };
            edges.borrow_mut().push(EdgeCell {
                bounds,
                widths,
                colors,
                styles,
                source,
                doc_ix,
            });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}
