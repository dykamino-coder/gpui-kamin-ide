//! Пробы фона ячеек, рамки сетки и кромок: что красят слой фонов и художник кромок (`paint::table`).

use super::{CellEdges, EdgeCell};
use crate::layout::probe::Sink;
use gpui::{AnyElement, Bounds, Pixels};

/// Фоны ячеек сросшейся модели: прямоугольник и цвет, снятые пробой ячейки
/// на ПОДГОТОВКЕ кадра; красит их `CellBgPainter` — слой, лежащий в сетке
/// ПЕРЕД кромками и ячейками. Так фон ячейки оказывается под кромками, а
/// содержимое ячейки — над ними, как у Blink: сросшиеся кромки идут в фазе
/// `kDescendantBlockBackgroundsOnly` (`box_fragment_painter.cc:952-957`,
/// «Collapsed borders paint *after* children have painted their
/// backgrounds»), а строчное, плавающее и позиционированное содержимое
/// ячеек — в более поздних фазах, то есть поверх кромок.
pub type CellBgs = Sink<(Bounds<Pixels>, gpui::Hsla)>;

/// Проба фона ячейки: холст во всю коробку ячейки записывает её рамку и
/// цвет на подготовке кадра (та же механика, что `edge_probe`).
pub fn cell_bg_probe(bgs: CellBgs, colour: gpui::Hsla) -> AnyElement {
    bgs.record(move |bounds| (bounds, colour))
}

/// Проба кромок: как проба фона, пишет в PREPAINT границы и рамки ячейки.
/// Метка «коробка СЕТКИ»: проба ничего не рисует, а сообщает слою кромок,
/// где кончаются дорожки. Граница сетки есть ВСЕГДА, даже когда рисующей
/// кромки у таблицы нет, и смешивать эти два понятия нельзя: ни «крайние
/// линии только с `source == 0`», ни «нулевая проба таблицы» без этой
/// границы не обходятся.
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
    edges.record_with(move |g, window| {
        let bounds = g.origin_aligned(gpui::px(0.5 / window.scale_factor() + 1e-4));
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
        EdgeCell {
            bounds,
            widths,
            colors,
            styles,
            source,
            doc_ix,
        }
    })
}
