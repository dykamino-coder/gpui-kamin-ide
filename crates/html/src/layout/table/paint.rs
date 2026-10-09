//! Покраска фонов ячеек и рамок таблицы.
// owner: A

use crate::text::clamp::forget_clamp_buffers;
use gpui::{AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId, Pixels, Styled, Window, px};

/// Отрисовка ребёнка только в ПРЯМОУГОЛЬНИКАХ, снятых пробами прошлого кадра.
///
/// Фон ряда таблицы (css-tables-3 §drawing-backgrounds): картинка ряда
/// рисуется В ЯЧЕЙКАХ, непрерывно от начала ряда, а зазоры остаются чистыми.
/// Геометрию ячеек знает только раскладка — её снимают пробы в ячейках, а
/// ряд рисует своего ребёнка по разу на прямоугольник, обрезая маской.
/// Первый кадр пуст (пробы ещё не писали) — стенд и так ждёт устоявшийся.
/// Прямоугольник ячейки + флаг «точная»: точная лежит целиком в своём
/// ряду/колонке (span = 1), объединённая (rowspan/colspan) выходит за них.
/// Область фона считается ТОЛЬКО по точным — объединённая растягивала бы
/// градиент колонки на чужие дорожки; маски краски — по всем. Третье поле —
/// тот же прямоугольник без округления к точке устройства: от него
/// считается область позиционирования (CSS 2.1 §17.5.1, Blink — по
/// неокруглённой геометрии ячеек), маски остаются округлёнными.
pub type RowRects = std::rc::Rc<std::cell::RefCell<Vec<(Bounds<Pixels>, bool, Bounds<Pixels>)>>>;

thread_local! {
    /// Буферы прямоугольников ПО РЯДАМ, переживающие перестройку дерева:
    /// каждый кадр стенд строит элементы заново, и Rc из прошлого кадра
    /// иначе терялся вместе с записями проб.
    pub(crate) static ROW_RECTS: std::cell::RefCell<std::collections::HashMap<u64, RowRects>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Буфер прямоугольников ряда по устойчивому номеру узла.
pub fn row_rects_for(node_id: u64) -> RowRects {
    ROW_RECTS.with(|m| m.borrow_mut().entry(node_id).or_default().clone())
}

/// Сброс буферов проб при смене документа.
///
/// Номера узлов считаются с нуля в каждом документе: без сброса полоса
/// нового документа забирала прямоугольники ячеек ПРЕЖНЕГО с тем же
/// номером, и первый кадр красил фон по чужим местам — а если ничего не
/// инвалидировало окно, грязный кадр оставался последним.
pub fn forget_row_rects() {
    ROW_RECTS.with(|m| m.borrow_mut().clear());
    CELL_EDGES.with(|m| m.borrow_mut().clear());
    BAND_RETRIES.with(|m| m.borrow_mut().clear());
    forget_clamp_buffers();
}

thread_local! {
    /// Сколько кадров полоса фона прождала своих проб. Ключ — адрес буфера
    /// проб.
    ///
    /// Полоса без единой ячейки (`<col>` без рядов, `<col>` за краем сетки)
    /// не дождётся их никогда, а запрос кадра без счётчика вертел бы окно
    /// вечно: документ не успокаивается, и стенд снимает его на таймауте.
    pub(crate) static BAND_RETRIES: std::cell::RefCell<std::collections::HashMap<usize, u8>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Сколько кадров ждать пробы, прежде чем счесть полосу пустой.
pub(crate) const BAND_WAIT_FRAMES: u8 = 2;

pub struct CellsClipped {
    pub(crate) style: crate::style::computed::Computed,
    pub(crate) rects: RowRects,
}

impl CellsClipped {
    pub fn new(rects: RowRects, style: crate::style::computed::Computed) -> Self {
        CellsClipped { style, rects }
    }
}

impl Element for CellsClipped {
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
        // Оверлей вне потока: раскладку таблицы полоса не трогает.
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
        // Пробы ячеек пишут в PREPAINT, а вся подготовка кадра идёт до
        // отрисовки — здесь забираются прямоугольники ЭТОГО ЖЕ кадра.
        // Пустота возможна только на самом первом кадре документа.
        let rects = std::mem::take(&mut *self.rects.borrow_mut());
        if rects.is_empty() {
            // Пробы ячеек ещё не писали (первый кадр) — без нового кадра
            // окно не перерисуется, и фон не появится никогда. Но ждать
            // бесконечно нельзя: у полосы может не быть ячеек вовсе.
            let key = std::rc::Rc::as_ptr(&self.rects) as usize;
            let waited = BAND_RETRIES.with(|m| {
                let mut m = m.borrow_mut();
                let n = m.entry(key).or_insert(0);
                *n = n.saturating_add(1);
                *n
            });
            if waited <= BAND_WAIT_FRAMES {
                window.request_animation_frame();
            }
            return;
        }
        BAND_RETRIES.with(|m| {
            m.borrow_mut()
                .remove(&(std::rc::Rc::as_ptr(&self.rects) as usize));
        });
        // Область ряда/колонки — охват ТОЧНЫХ ячеек (span = 1): от неё
        // считается и размер плитки, и `background-position`. Объединённые
        // лежат и на чужих дорожках — они только маски.
        let union = |pick: &dyn Fn(&(Bounds<Pixels>, bool, Bounds<Pixels>)) -> Bounds<Pixels>| {
            let exact: Vec<Bounds<Pixels>> = rects.iter().filter(|r| r.1).map(pick).collect();
            let all: Vec<Bounds<Pixels>> = rects.iter().map(pick).collect();
            let base = if exact.is_empty() { all } else { exact };
            let mut area = base[0];
            for r in &base[1..] {
                let right = area.origin.x + area.size.width;
                let bottom = area.origin.y + area.size.height;
                let x0 = area.origin.x.min(r.origin.x);
                let y0 = area.origin.y.min(r.origin.y);
                let x1 = right.max(r.origin.x + r.size.width);
                let y1 = bottom.max(r.origin.y + r.size.height);
                area = Bounds {
                    origin: gpui::point(x0, y0),
                    size: gpui::size(x1 - x0, y1 - y0),
                };
            }
            area
        };
        // Тень и обводка — по округлённым ячейкам (резкие края); фон
        // позиционируется по неокруглённым (CSS 2.1 §17.5.1, как у Blink).
        let area = union(&|r| r.0);
        let positioning = union(&|r| r.2);
        let all: Vec<Bounds<Pixels>> = rects.iter().map(|(b, _, _)| *b).collect();
        // Тень РЯДА — вокруг охвата всех его ячеек, без маски: она лежит
        // снаружи. Резкая (без размытия) рисуется кольцевым квадом — тот же
        // обход вырождения шейдера, что у обычных коробок.
        for sh in &self.style.shadows {
            let colour = if sh.color.a < 0.0 {
                self.style.color.unwrap_or(crate::style::values::value::Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                })
            } else {
                sh.color
            };
            let shifted = Bounds {
                origin: gpui::point(
                    area.origin.x + gpui::px(sh.x),
                    area.origin.y + gpui::px(sh.y),
                ),
                size: area.size,
            };
            if sh.blur > 0.0 {
                // Коробка — сам охват, смещение — в тени: примитив вырезает
                // тень под СВОЕЙ коробкой (патч gpui `Shadow::box_bounds`),
                // и сдвинутый охват вырезал бы не то место.
                window.paint_drop_shadows(
                    area,
                    gpui::Corners::default(),
                    &[gpui::BoxShadow {
                        color: colour.to_hsla(),
                        offset: gpui::point(gpui::px(sh.x), gpui::px(sh.y)),
                        // σ = половина радиуса CSS (как в `apply::apply_paint`).
                        blur_radius: gpui::px(sh.blur * 0.5),
                        spread_radius: gpui::px(sh.spread),
                        inset: false,
                    }],
                );
            } else {
                let grown = Bounds {
                    origin: gpui::point(
                        shifted.origin.x - gpui::px(sh.spread),
                        shifted.origin.y - gpui::px(sh.spread),
                    ),
                    size: gpui::size(
                        shifted.size.width + gpui::px(sh.spread * 2.0),
                        shifted.size.height + gpui::px(sh.spread * 2.0),
                    ),
                };
                let mut quad = gpui::fill(grown, gpui::transparent_black());
                quad.border_color = colour.to_hsla();
                quad.border_widths = gpui::Edges {
                    top: gpui::px((sh.spread - sh.y).max(0.0)),
                    right: gpui::px((sh.spread + sh.x).max(0.0)),
                    bottom: gpui::px((sh.spread + sh.y).max(0.0)),
                    left: gpui::px((sh.spread - sh.x).max(0.0)),
                };
                window.paint_quad(quad);
            }
        }
        // Обводка ряда/группы — вокруг охвата ТОЧНЫХ ячеек, снаружи и без
        // маски (css-ui-4 §outline: рамка вне коробки, раскладку не трогает).
        // Тот же кольцевой квад, что у резкой тени выше.
        if let Some(o) = &self.style.outline {
            let em = match self.style.font_size {
                Some(crate::style::values::value::Len::Px(v)) => v,
                _ => 16.0,
            };
            let px_of = |l: Option<crate::style::values::value::Len>| match l {
                Some(crate::style::values::value::Len::Px(v)) => v,
                Some(crate::style::values::value::Len::Em(k)) => k * em,
                _ => 0.0,
            };
            let w = px_of(o.width);
            let out = px_of(o.offset) + w;
            if let (true, true, Some(colour)) =
                (o.style != Some(0), w > 0.0, o.color.or(self.style.color))
            {
                let ring = Bounds {
                    origin: gpui::point(area.origin.x - gpui::px(out), area.origin.y - gpui::px(out)),
                    size: gpui::size(
                        area.size.width + gpui::px(2.0 * out),
                        area.size.height + gpui::px(2.0 * out),
                    ),
                };
                let mut quad = gpui::fill(ring, gpui::transparent_black());
                quad.border_color = colour.to_hsla();
                quad.border_widths = gpui::Edges {
                    top: gpui::px(w),
                    right: gpui::px(w),
                    bottom: gpui::px(w),
                    left: gpui::px(w),
                };
                window.paint_quad(quad);
            }
        }
        for rect in all {
            window.with_content_mask(Some(gpui::ContentMask { bounds: rect }), |window| {
                // Цвет ряда — под картинкой, в тех же прямоугольниках: на
                // ячейки его в этом случае не переносят (иначе он закрашивал
                // бы картинку, рисуясь позже полосы).
                if let Some(bg) = self.style.background {
                    window.paint_quad(gpui::fill(rect, bg.to_hsla()));
                }
                crate::paint::background::paint_area(&self.style, positioning, window);
            });
        }
    }
}

impl IntoElement for CellsClipped {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Сросшиеся кромки таблицы: ячейка без собственных рамок отдаёт их
/// отдельному слою — кромки рисуются НА ЛИНИЯХ сетки поверх фонов
/// (css-tables-3 §drawing-borders), а конфликт «шире побеждает»
/// (CSS 2.1 §17.6.2.1) решается порядком: узкие раньше, широкие поверх.
pub struct EdgeCell {
    pub bounds: Bounds<Pixels>,
    /// Ширины кромок [верх, право, низ, лево] в точках.
    pub widths: [f32; 4],
    pub colors: [crate::style::values::value::Color; 4],
    /// Ранги стилей сторон (см. `Computed::border_side_styles`); 9 = solid.
    pub styles: [u8; 4],
    /// Ранг источника (CSS 2.1 §17.6.2.1 п.4), больше — сильнее: таблица 0,
    /// группа колонок 1, колонка 2, группа рядов 3, ряд 4, ячейка 5.
    pub source: u8,
    /// Порядок в документе: раньше = выше/левее, при равенстве побеждает.
    pub doc_ix: u32,
}

pub type CellEdges = std::rc::Rc<std::cell::RefCell<Vec<EdgeCell>>>;

thread_local! {
    pub(crate) static CELL_EDGES: std::cell::RefCell<std::collections::HashMap<u64, CellEdges>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

pub fn cell_edges_for(key: u64) -> CellEdges {
    CELL_EDGES.with(|m| m.borrow_mut().entry(key).or_default().clone())
}

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
        // Кандидат кромки на ЛИНИИ сетки: совпадающие отрезки соседей — ОДНА
        // кромка, победитель по CSS 2.1 §17.6.2.1 (hidden гасит всех, затем
        // шире, ранг стиля, источник ячейка>таблица, порядок в документе).
        struct Cand {
            line: f32,
            a: f32,
            b: f32,
            w: f32,
            style: u8,
            source: u8,
            doc_ix: u32,
            colour: crate::style::values::value::Color,
            /// Наружная сторона крайней линии таблицы (-1/1); 0 — центр.
            outward: i8,
        }
        let mut vert: Vec<Cand> = vec![];
        let mut horiz: Vec<Cand> = vec![];
        for c in &cells {
            if c.source == GRID_BOX {
                continue;
            }
            let bnd = c.bounds;
            let (x0, y0) = (f32::from(bnd.origin.x), f32::from(bnd.origin.y));
            let (x1, y1) = (
                x0 + f32::from(bnd.size.width),
                y0 + f32::from(bnd.size.height),
            );
            let is_table = c.source == 0;
            let mut side = |list: &mut Vec<Cand>, line: f32, a: f32, b: f32, i: usize, out: i8| {
                if c.widths[i] > 0.0 || c.styles[i] == 1 {
                    list.push(Cand {
                        line,
                        a,
                        b,
                        w: c.widths[i],
                        style: c.styles[i],
                        source: c.source,
                        doc_ix: c.doc_ix,
                        colour: c.colors[i],
                        outward: if is_table { out } else { 0 },
                    });
                }
            };
            side(&mut horiz, y0, x0, x1, 0, -1);
            side(&mut vert, x1, y0, y1, 1, 1);
            side(&mut horiz, y1, x0, x1, 2, 1);
            side(&mut vert, x0, y0, y1, 3, -1);
        }
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
        type SegKey = (f32, u8, u8, u32);
        let mut segs: Vec<(SegKey, bool, Bounds<Pixels>, crate::style::values::value::Color)> = Vec::new();
        let mut draw = |cands: &mut Vec<Cand>, vertical: bool, grid_lo: Option<f32>| {
            cands.sort_by(|p, q| {
                p.line
                    .partial_cmp(&q.line)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            // Крайние линии таблицы: кромка не центрируется, а рисуется
            // внутрь бокса (наружная половина у браузеров уходит в поля,
            // эталоны считают рамку частью коробки).
            // Начало сетки — не «первая нарисованная кромка», а край ДОРОЖЕК.
            // У браузера коробка таблицы раздаётся наружу на половину
            // победившей кромки, а сама кромка красится центрировано. Выноса
            // у нас нет: сетка стоит там, где у браузера ВНЕШНИЙ край
            // коробки, — поэтому кромку НАЧАЛЬНОЙ линии вжимаем внутрь, её
            // наружная половина и занимает недостающий вынос. Конец сетки
            // координату не сдвигает: там центр.
            let lo_line = grid_lo.unwrap_or_else(|| cands.first().map(|c| c.line).unwrap_or(0.0));
            let mut i = 0;
            while i < cands.len() {
                let mut j = i + 1;
                while j < cands.len() && (cands[j].line - cands[i].line).abs() < 0.75 {
                    j += 1;
                }
                let group = &cands[i..j];
                let outward = group
                    .iter()
                    .find_map(|c| (c.outward != 0).then_some(c.outward));
                let mut cuts: Vec<f32> = group.iter().flat_map(|c| [c.a, c.b]).collect();
                cuts.sort_by(|p, q| p.partial_cmp(q).unwrap_or(std::cmp::Ordering::Equal));
                cuts.dedup_by(|p, q| (*p - *q).abs() < 0.5);
                for seg in cuts.windows(2) {
                    let (a, b) = (seg[0], seg[1]);
                    if b - a < 0.5 {
                        continue;
                    }
                    let mid = (a + b) / 2.0;
                    let covering: Vec<&Cand> = group
                        .iter()
                        .filter(|c| c.a - 0.25 <= mid && mid <= c.b + 0.25)
                        .collect();
                    if covering.is_empty() || covering.iter().any(|c| c.style == 1) {
                        continue;
                    }
                    let win = covering
                        .iter()
                        .max_by(|p, q| {
                            let kp = (p.w, p.style, p.source, u32::MAX - p.doc_ix);
                            let kq = (q.w, q.style, q.source, u32::MAX - q.doc_ix);
                            kp.partial_cmp(&kq).unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .unwrap();
                    if win.w <= 0.0 || win.colour.a == 0.0 {
                        continue;
                    }
                    let line = win.line;
                    // Кромка стола лежит теперь на ТОЙ ЖЕ линии сетки, что и
                    // кромки краевых ячеек, поэтому центрируются ОБЕ
                    // (§17.6.2). Проверка: коробка стола начинается на
                    // `линия − outer_win/2`, победившая полоса шириной `w`
                    // занимает `линия ± w/2`, и при `w == outer_win` это ровно
                    // `край … край + w` — прежний вжим внутрь давал ту же
                    // полосу побайтно. При более узкой рамке стола полоса
                    // стола и не рисуется: линию забирает более широкая ячейка.
                    // Вжим по `lo_line` остаётся там, где пробы стола в группе
                    // нет вовсе: у такой таблицы коробка совпадает с внешними
                    // краями ячеек (на этом держится замер «нулевая проба
                    // таблицы», CSS2 +21/−25).
                    let edge_dir = if outward.is_none() && (line - lo_line).abs() < 0.75 {
                        Some(1)
                    } else {
                        None
                    };
                    let (lo, hi) = match edge_dir {
                        Some(-1) => (line - win.w, line),
                        Some(_) => (line, line + win.w),
                        None => (line - win.w / 2.0, line + win.w / 2.0),
                    };
                    // Продление В УГЛЫ — только у сплошных (`style >= 9`):
                    // пересечение иначе оставалось пустым квадратом, а
                    // продление пунктирных рисовало лишние усы. Обе оси
                    // тянутся на полуширину ПЕРПЕНДИКУЛЯРНОЙ кромки; кто из
                    // них накроет угол, решает порядок по ключу (см. `segs`).
                    let (a, b) = if win.style >= 9 {
                        if vertical {
                            (a - half_at_h(a, line), b + half_at_h(b, line))
                        } else {
                            (a - half_at(a, line), b + half_at(b, line))
                        }
                    } else {
                        (a, b)
                    };
                    let rect = if vertical {
                        Bounds {
                            origin: gpui::point(gpui::px(lo), gpui::px(a)),
                            size: gpui::size(gpui::px(hi - lo), gpui::px(b - a)),
                        }
                    } else {
                        Bounds {
                            origin: gpui::point(gpui::px(a), gpui::px(lo)),
                            size: gpui::size(gpui::px(b - a), gpui::px(hi - lo)),
                        }
                    };
                    segs.push((
                        (win.w, win.style, win.source, u32::MAX - win.doc_ix),
                        vertical,
                        rect,
                        win.colour,
                    ));
                }
                i = j;
            }
        };
        draw(&mut vert, true, grid_lo.map(|g| g.0));
        draw(&mut horiz, false, grid_lo.map(|g| g.1));
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

/// Проба ячейки: канвас, записывающий свои границы для фона ряда.
///
/// `border` — ширины сторон ячейки в порядке верх-право-низ-лево. Абсолютный
/// ребёнок в taffy лежит ВНУТРИ рамки, поэтому канвас меряет поле подкладки,
/// а §17.5.1 велит вести фон полосы «from the top of the cells to the bottom
/// of the cells», то есть по внешним краям рамок: ячейка с
/// `border-bottom: 60px` и пустым содержимым давала полосе нулевую высоту.
pub fn cell_rect_probe(
    rects: RowRects,
    exact: bool,
    shift: (f32, f32),
    border: [f32; 4],
) -> AnyElement {
    CellProbe {
        child: Some(
            gpui::div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .into_any_element(),
        ),
        rects,
        exact,
        shift,
        border,
    }
    .into_any_element()
}

/// Проба ячейки (`cell_rect_probe`): записывает и округлённый прямоугольник
/// (маски краски), и неокруглённый (область позиционирования фона полосы).
/// Поле подкладки ячейки округляется от её внутреннего края рамки: 25px
/// рамки при 1.25 сдвигали его на 0.2px, и плитка `top right` у tbody
/// вставала на точку правее эталона (`background-position-applies-to-001a`).
pub(crate) struct CellProbe {
    pub(crate) child: Option<AnyElement>,
    pub(crate) rects: RowRects,
    pub(crate) exact: bool,
    pub(crate) shift: (f32, f32),
    pub(crate) border: [f32; 4],
}

impl CellProbe {
    pub(crate) fn outer(&self, bounds: Bounds<Pixels>) -> Bounds<Pixels> {
        let (shift, border) = (self.shift, self.border);
        // Сдвиг краски относительно коробки ячейки: в сросшейся модели
        // фоновая сетка начинается от середины рамки таблицы.
        Bounds {
            origin: gpui::point(
                bounds.origin.x + gpui::px(shift.0 - border[3]),
                bounds.origin.y + gpui::px(shift.1 - border[0]),
            ),
            size: gpui::size(
                bounds.size.width + gpui::px(border[1] + border[3]),
                bounds.size.height + gpui::px(border[0] + border[2]),
            ),
        }
    }
}

impl Element for CellProbe {
    type RequestLayoutState = LayoutId;
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
    ) -> (LayoutId, LayoutId) {
        let layout_id = self.child.as_mut().unwrap().request_layout(window, cx);
        (layout_id, layout_id)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout_id: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Запись В PREPAINT: подготовка ВСЕХ элементов идёт до отрисовки,
        // и полоса фона читает прямоугольники СВОЕГО кадра — с записью в
        // paint она рисовала прошлый кадр и мигала на каждой смене раскладки.
        let unrounded = Bounds {
            origin: window.layout_origin_unrounded(*layout_id),
            size: window.layout_size_unrounded(*layout_id),
        };
        // Неокруглённое берётся, только пока оно в пределах точки
        // устройства от округлённого (иначе — другой кадр отсчёта).
        let near = |a: Pixels, b: Pixels| (a - b).abs() <= px(1.0);
        let unrounded = if near(unrounded.left(), bounds.left())
            && near(unrounded.top(), bounds.top())
            && near(unrounded.right(), bounds.right())
            && near(unrounded.bottom(), bounds.bottom())
        {
            unrounded
        } else {
            bounds
        };
        self.rects
            .borrow_mut()
            .push((self.outer(bounds), self.exact, self.outer(unrounded)));
        self.child.as_mut().unwrap().prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut LayoutId,
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.as_mut().unwrap().paint(window, cx);
    }
}

impl IntoElement for CellProbe {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
