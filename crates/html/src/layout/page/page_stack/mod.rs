//! Стопка страниц `PageStack` — GPUI-элемент постраничного документа: листы сеткой
//! с масштабом до вмещения, page area каждого — фрагментаинер (css-break-4 §2). Здесь
//! геометрия листа `PageGeom`, описание марджин-бокса `MarginBox`, дети `PageKid`, слои
//! ICB и `position: fixed` по копии на лист, план фрагментов и маски page area;
//! укладка, листы, марджин-боксы, канвас и ось письма — в подмодулях.

use crate::layout::fragment::plan::Plan;
use gpui::{AnyElement, Bounds, Pixels, point, px, size};
mod canvas;
pub(crate) use canvas::{CanvasLayerFn, builder as canvas_builder, image_layers};
mod axis;
mod contents;
mod continuation;
mod element;
mod kids;
mod measure;
mod oof;
mod pages;
mod planning;
mod regrow;
mod resume;
mod sheets;
mod spaces;

/// Лист страницы (css-page-3 §page-model): полный размер, поля, рамка и
/// отступы (верх/право/низ/лево), фон листа, канвас документа и page area —
/// контентная область листа, она же фрагментаинер.
#[derive(Clone, Copy, Debug)]
pub struct PageGeom {
    pub size: (f32, f32),
    pub margin: [f32; 4],
    pub border: (f32, gpui::Hsla),
    /// Контур листа: толщина, сдвиг наружу от рамки, цвет (css-ui-4 §outline;
    /// у коробки страницы это обычное свойство, `page-box-010`).
    pub outline: (f32, f32, gpui::Hsla),
    pub padding: [f32; 4],
    /// Фон листа — кроет ВЕСЬ лист вместе с полями (§painting, слой 1).
    pub bg: gpui::Hsla,
    /// Канвас документа — фон `html`/`body`; кроет border box листа (слой 2).
    pub canvas: Option<gpui::Hsla>,
    pub area: (f32, f32),
    /// `page-orientation` (css-page-3 §page-orientation-prop): лист
    /// раскладывается как обычно и показывается повёрнутым — 1 на четверть
    /// оборота вправо (`rotate-right`), 3 — влево (`rotate-left`), 0 — нет.
    pub turn: u8,
}

impl PageGeom {
    /// Размер листа, каким его видно (после `page-orientation`).
    pub(crate) fn shown(&self) -> (f32, f32) {
        if self.turn % 2 == 1 {
            (self.size.1, self.size.0)
        } else {
            self.size
        }
    }

    /// Левый верх page area внутри листа.
    pub(crate) fn area_origin(&self) -> (f32, f32) {
        (
            self.margin[3] + self.border.0 + self.padding[3],
            self.margin[0] + self.border.0 + self.padding[0],
        )
    }
}

mod kid;
pub(crate) use kid::LeafPaint;
pub use kid::PageKid;

/// Геометрия листа по его номеру (с нуля) и имени страницы: каскад
/// `@page` (`:first`, `:left`/`:right`, имена) решает стенд.
pub type PageGeomFn = std::rc::Rc<dyn Fn(usize, &str) -> PageGeom>;

/// Порождённый марджин-бокс листа (css-page-3 §margin-boxes): элемент во весь
/// border box, его копия для меры по содержимому, заданные размеры (border box,
/// `None` — `auto`) и поля (`None` — `auto`), верх/право/низ/лево.
pub struct MarginBox {
    pub place: crate::layout::page::margin_layout::Place,
    /// Элемент коробки по её border box (ширина, высота) — строится после
    /// раскладки, с размером в точках.
    pub make: std::rc::Rc<dyn Fn(f32, f32) -> AnyElement>,
    pub probe: std::rc::Rc<dyn Fn(Option<crate::style::values::value::Len>) -> AnyElement>,
    pub vertical: bool,
    pub w: Option<f32>,
    pub h: Option<f32>,
    pub intrinsic: [Option<crate::style::values::value::Len>; 2],
    pub margin: [Option<f32>; 4],
}

/// Марджин-боксы листа по номеру, имени, числу листов и геометрии.
pub type MarginFn = std::rc::Rc<dyn Fn(usize, &str, usize, &PageGeom) -> Vec<MarginBox>>;

/// Стопка страниц: page area каждой — фрагментаинер (css-break-4 §2). Листы
/// раскладываются сеткой и МАСШТАБИРУЮТСЯ до вмещения в свою коробку: стенд
/// сравнивает кадр целиком, а печатный эталон WPT тоже многостраничен, и
/// сравнивать надо все страницы обеих сторон.
pub struct PageStack {
    complete: std::cell::Cell<bool>,
    pub(crate) kids: Vec<PageKid>,
    measured: std::collections::HashMap<(usize, usize), continuation::CopyGeometry>,
    continuations: std::collections::HashMap<(usize, usize), crate::layout::fragment::push::Regrow>,
    /// Лист `i` с именем страницы — его геометрия (css-page-3 §cascading).
    pub(crate) geom_for: PageGeomFn,
    /// Геометрия КАЖДОГО листа, итог `prepaint` (листы бывают разные:
    /// `@page :first { size }`, именные страницы).
    pub(crate) geoms: std::cell::RefCell<Vec<PageGeom>>,
    pub(crate) cell: std::cell::Cell<(f32, f32)>,
    /// Марджин-боксы: построитель и итог раскладки `(лист, элемент)`.
    pub(crate) margin_for: Option<MarginFn>,
    pub(crate) margin_els: Vec<(usize, AnyElement)>,
    /// Слой начального содержащего блока (внепоточные без позиционированного
    /// предка) — по КОПИИ на страницу: `icb[p]` рисуется на листе `p` со
    /// сдвигом на `p` page area вверх, то есть абсолют раскладывается «как
    /// непрерывный поток» и режется страницами (css-position-3
    /// §abspos-breaking; `monolithic-overflow-013`: текст после монолита
    /// 350vh — в середине четвёртой страницы). Повтор `position: fixed` на
    /// каждой странице без сдвига — шаг 3.
    pub(crate) icb: Vec<Vec<AnyElement>>,
    /// Досягаемость абсолютов корня — низ самого дальнего (с переполнением
    /// монолитов, как Blink `ReserveSpaceForMonolithicOverflow`): листов не
    /// меньше, чем нужно, чтобы её показать.
    pub(crate) icb_reach: f32,
    /// Слой `position: fixed` — по копии на страницу БЕЗ сдвига (Blink
    /// `IsMonolithic`: fixed при печати повторяется на каждой странице).
    pub(crate) fixed: Vec<Vec<AnyElement>>,
    pub(crate) plan: std::cell::RefCell<Plan>,
    oof: oof::State,
    pub(crate) pages: std::cell::Cell<usize>,
    pub(crate) grid: std::cell::Cell<(usize, f32)>,
    /// Показываемые листы (с нуля, по возрастанию), `None` — все. Печатный
    /// reftest WPT сравнивает только страницы из `<meta name=reftest-pages>`.
    pub(crate) select: Option<Vec<usize>>,
    /// Image layers of the document canvas and their per-sheet layout.
    pub(crate) canvas_layer: Option<CanvasLayerFn>,
    pub(crate) canvas_els: Vec<(usize, AnyElement)>,
    /// Block axis of the root writing mode (`page_stack/axis.rs`).
    pub(crate) axis: crate::layout::fragment::types::StackAxis,
    principal: crate::style::computed::writing_mode::WritingMode,
}

impl PageStack {
    pub fn new(
        kids: Vec<PageKid>,
        geom_for: PageGeomFn,
        icb: Vec<Vec<AnyElement>>,
        icb_reach: f32,
        fixed: Vec<Vec<AnyElement>>,
        margin_for: Option<MarginFn>,
    ) -> Self {
        PageStack {
            complete: std::cell::Cell::new(false),
            kids,
            measured: Default::default(),
            continuations: Default::default(),
            geom_for,
            geoms: std::cell::RefCell::new(Vec::new()),
            cell: std::cell::Cell::new((1.0, 1.0)),
            margin_for,
            margin_els: Vec::new(),
            icb,
            icb_reach,
            fixed,
            plan: std::cell::RefCell::new(Plan::default()),
            oof: Default::default(),
            pages: std::cell::Cell::new(1),
            grid: std::cell::Cell::new((1, 1.0)),
            select: None,
            canvas_layer: None,
            canvas_els: Vec::new(),
            axis: crate::layout::fragment::types::StackAxis::Horizontal,
            principal: Default::default(),
        }
    }

    /// Последняя предраскладка получила согласованный план; incomplete не рисуется.
    pub fn has_complete_plan(&self) -> bool {
        self.complete.get()
            && self.plan.borrow().completion == crate::layout::fragment::plan::Completion::Complete
    }

    pub fn with_select(mut self, select: Option<Vec<usize>>) -> Self {
        self.select = select;
        self
    }

    /// Место листа `i` в сетке показываемых; `None` — лист не показывается.
    pub(crate) fn slot(&self, i: usize) -> Option<usize> {
        match &self.select {
            None => Some(i),
            Some(sel) => sel.iter().position(|&p| p == i),
        }
    }

    /// Левый верх листа `i` в НЕмасштабированных точках стопки.
    pub(crate) fn sheet_origin(&self, i: usize) -> (f32, f32) {
        let Some(i) = self.slot(i) else {
            return (-1.0e6, -1.0e6);
        };
        let per_row = self.grid.get().0.max(1);
        let (cw, ch) = self.cell.get();
        ((i % per_row) as f32 * cw, (i / per_row) as f32 * ch)
    }

    /// Геометрия листа `i` (итог `prepaint`; за краем — последний лист).
    pub(crate) fn geom(&self, i: usize) -> PageGeom {
        let gs = self.geoms.borrow();
        gs.get(i)
            .or(gs.last())
            .copied()
            .unwrap_or_else(|| (self.geom_for)(i, ""))
    }

    /// Прямоугольник page area листа `i` в ИТОГОВЫХ координатах окна (с
    /// масштабом): шейдер режет по маске после преобразования
    /// (`shaders.hlsl` `distance_from_clip_rect_transformed`).
    pub(crate) fn area_mask(&self, bounds: Bounds<Pixels>, i: usize) -> gpui::ContentMask<Pixels> {
        let s = self.grid.get().1;
        let (sx, sy) = self.sheet_origin(i);
        let g = self.geom(i);
        let (ax, ay) = g.area_origin();
        let area = Bounds {
            origin: point(
                bounds.origin.x + px((sx + ax) * s),
                bounds.origin.y + px((sy + ay) * s),
            ),
            size: size(px(g.area.0 * s), px(g.area.1 * s)),
        };
        // Отрицательные поля выносят page area ЗА лист, а видно только то,
        // что на листе (`page-margin-negative-print.tentative`: красная рамка
        // 20px на −20..0 обязана уйти под обрез). Маски фрагментов, слоёв ICB
        // и `fixed` — пересечения с этой, обрез достаётся всем.
        let sheet = Bounds {
            origin: point(bounds.origin.x + px(sx * s), bounds.origin.y + px(sy * s)),
            size: size(px(g.size.0 * s), px(g.size.1 * s)),
        };
        gpui::ContentMask {
            bounds: area.intersect(&sheet),
        }
    }
}
