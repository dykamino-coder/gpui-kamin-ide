//! Художник сросшихся кромок таблицы: отрезки-победители красятся по ключу с привязкой к пикселям.

use super::edge_segments::{Seg, collect_cands, draw_cands};
use super::{CellEdges, GRID_BOX};
use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    Pixels, Window,
};

/// Слой сросшихся кромок: рисует рамки всех ячеек таблицы, центрируя
/// каждую на границе ячейки. Совпадающие кромки соседей ложатся друг на
/// друга; побеждает нарисованная позже — порядок по ширине даёт правило
/// «шире побеждает».
pub struct EdgePainter {
    pub(crate) edges: CellEdges,
}

// ★ ЗАМЕРЕНО И ОТКАЧЕНО (11.09, `scout-collapsedborders-2026-09.md`,
// 5 хунков): маска «do-not-fill» для ячейки с охватом — ячейка отдаёт
// слою кромок свой прямоугольник, и линию строго внутри него не
// заливают ни ряд, ни группа, ни колонка, ни стол (1:1 Blink
// `MarkInnerBordersAsDoNotFill`, `table_borders.cc:555`; набор S по
// css-tables-3 строится только из `table-cell`).
// Срез 246 пар семьи, база тем же списком: 228 -> 228, **+0 / −0**.
// Модель верна, но ни одной пары не двигает: конфликт §17.6.2.1
// разбирается ДВАЖДЫ — в раскладке (`render.rs: win_edges`, только
// ячейки и стол, простой `max()`) и здесь, по всем шести источникам.
// Смысл появится, когда обе модели сведут в одну, как `TableBorders`
// у Blink; порознь маска — мёртвый код.
impl EdgePainter {
    pub fn new(edges: CellEdges) -> Self {
        EdgePainter { edges }
    }
}

impl Element for EdgePainter {
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
        let cells = std::mem::take(&mut *self.edges.borrow_mut());
        // Начало сетки по каждой оси — отдельно от того, кто эту линию красит.
        let grid_lo = cells
            .iter()
            .find(|c| c.source == GRID_BOX)
            .map(|c| (f32::from(c.bounds.origin.x), f32::from(c.bounds.origin.y)));
        let (mut vert, mut horiz) = collect_cands(&cells);
        // Снимок вертикалей для стыков: горизонталь тянется в угол на
        // половину ВЕРТИКАЛЬНОЙ кромки, а не своей (§17.6.2: кромки
        // центрированы на линиях сетки, и ширина стыка задаётся
        // перпендикуляром). Своя полуширина рисовала ус там, где вертикали
        // нет вовсе: `border-top-width: 96px` вылезал на 48 точек за край.
        let vert_spans: Vec<(f32, f32, f32, f32, u8)> = vert
            .iter()
            .map(|c| (c.line, c.a, c.b, c.w, c.style))
            .collect();
        let half_at = |x: f32, y: f32| -> f32 {
            let mut widest = 0.0f32;
            for c in &vert_spans {
                if (c.0 - x).abs() >= 0.75 || y < c.1 - 0.25 || y > c.2 + 0.25 {
                    continue;
                }
                // Погашенная вертикаль не рисуется, значит и заливать под
                // неё угол нечем.
                if c.4 == 1 {
                    return 0.0;
                }
                widest = widest.max(c.3);
            }
            widest / 2.0
        };
        // Симметрично для вертикалей: в стык вертикаль тянется на половину
        // ГОРИЗОНТАЛЬНОЙ кромки.
        let horiz_spans: Vec<(f32, f32, f32, f32, u8)> = horiz
            .iter()
            .map(|c| (c.line, c.a, c.b, c.w, c.style))
            .collect();
        let half_at_h = |y: f32, x: f32| -> f32 {
            let mut widest = 0.0f32;
            for c in &horiz_spans {
                if (c.0 - y).abs() >= 0.75 || x < c.1 - 0.25 || x > c.2 + 0.25 {
                    continue;
                }
                if c.4 == 1 {
                    return 0.0;
                }
                widest = widest.max(c.3);
            }
            widest / 2.0
        };
        // Стык кромок решается ПРИОРИТЕТОМ, а не осью: прежде горизонтали
        // рисовались ПОСЛЕ вертикалей и, протянутые в углы, всегда накрывали
        // стык своим цветом. У Blink стык достаётся кромке, победившей в
        // разборе §17.6.2.1 (`table_painters.cc`, `CollapsedBorderPainter`:
        // края отрезка подрезаются/растягиваются по соседней перпендикулярной
        // кромке в зависимости от того, кто сильнее). Поэтому отрезки обеих
        // осей копятся с ключом победителя и красятся по возрастанию ключа —
        // сильнейшая кромка ложится последней и забирает угол
        // (`border-conflict-element-001e`: синяя вертикаль первой ячейки
        // против жёлтой горизонтали второй — в эталоне угол синий).
        // При равном ключе вертикаль идёт первой — прежний порядок.
        let mut segs: Vec<Seg> = Vec::new();
        draw_cands(
            &mut vert,
            true,
            grid_lo.map(|g| g.0),
            &half_at,
            &half_at_h,
            &mut segs,
        );
        draw_cands(
            &mut horiz,
            false,
            grid_lo.map(|g| g.1),
            &half_at,
            &half_at_h,
            &mut segs,
        );
        segs.sort_by(|p, q| {
            p.0.partial_cmp(&q.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(p.1.cmp(&q.1).reverse())
        });
        // Snap every band edge to the device pixel grid the same way layout
        // bounds are rounded (`round()` of the absolute edge), so two bands
        // meeting at one exact coordinate share one device edge.
        let scale = window.scale_factor();
        let snap = |v: Pixels| gpui::px((f32::from(v) * scale).round() / scale);
        for (_, _, rect, colour) in segs {
            let x0 = snap(rect.origin.x);
            let y0 = snap(rect.origin.y);
            let x1 = snap(rect.origin.x + rect.size.width);
            let y1 = snap(rect.origin.y + rect.size.height);
            let rect = Bounds {
                origin: gpui::point(x0, y0),
                size: gpui::size(x1 - x0, y1 - y0),
            };
            window.paint_quad(gpui::fill(rect, colour.to_hsla()));
        }
    }
}

impl IntoElement for EdgePainter {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
