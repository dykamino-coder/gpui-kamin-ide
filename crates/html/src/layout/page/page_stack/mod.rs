//! Элемент стопки страниц `PageStack`.
// owner: A

use crate::layout::fragment::types::{Frag, Kid, Par, RepeatGeom};
use crate::layout::multicol::column_stack::ColumnStack;
use crate::layout::page::margin_boxes::layout_margin_boxes;
use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window, point, px, size,
};

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

/// Блок верхнего уровня документа в стопке страниц. Высота заранее НЕ
/// известна и меряется раскладкой (`layout_as_root`) в `prepaint` — так в
/// стопку попадает и голый текст, которого укладка колонок не видит.
pub struct PageKid {
    pub el: AnyElement,
    /// Копии на случай разреза между страницами (см. `StackChild::frags`).
    pub frags: Vec<AnyElement>,
    /// Монолит (css-break-4 §4.1 плюс `contain: size`, как Blink `IsMonolithic`).
    pub monolith: bool,
    /// Принудительный разрыв страницы перед/после (css-break-4 §3.1), включая
    /// смену имени страницы (css-page-3 §"Using named pages", п. 4).
    pub force_before: bool,
    pub force_after: bool,
    /// Мера поддерева от `render::shape_full` (css-break-4 §possible-breaks):
    /// высота в точках, точки законного разреза `(need, from)`, смещения
    /// принудительных разрывов и монолитные диапазоны — как у `StackChild`.
    /// `None` — высота не известна заранее, берётся измеренная, разрезов
    /// внутри нет.
    pub shape: Option<(f32, Vec<(f32, f32)>, Vec<f32>, Vec<(f32, f32)>)>,
    /// Имя страницы, с которого коробка начинается (css-page-3 §using-named-pages:
    /// start value), `""` — без имени. Лист, на котором коробка — первая, берёт
    /// это имя для своего `@page <имя>` (`PageStack::geom_for`).
    pub page: String,
    /// Поля коробки для укладки (схлопываются с соседями в `fill`) и смещение
    /// её border box внутри элемента — на него копия поднимается.
    pub mt: f32,
    pub mb: f32,
    pub inner_top: f32,
}

/// Геометрия листа по его номеру (с нуля) и имени страницы: каскад
/// `@page` (`:first`, `:left`/`:right`, имена) решает стенд.
pub type PageGeomFn = std::rc::Rc<dyn Fn(usize, &str) -> PageGeom>;

/// Порождённый марджин-бокс листа (css-page-3 §margin-boxes): элемент во весь
/// border box, его копия для меры по содержимому, заданные размеры (border box,
/// `None` — `auto`) и поля (`None` — `auto`), верх/право/низ/лево.
pub struct MarginBox {
    pub place: crate::page_margin::Place,
    /// Элемент коробки по её border box (ширина, высота) — строится после
    /// раскладки, с размером в точках.
    pub make: std::rc::Rc<dyn Fn(f32, f32) -> AnyElement>,
    pub probe: AnyElement,
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
    pub(crate) kids: Vec<PageKid>,
    /// Лист `i` с именем страницы — его геометрия (css-page-3 §cascading).
    pub(crate) geom_for: PageGeomFn,
    /// Геометрия КАЖДОГО листа, итог `prepaint` (листы бывают разные:
    /// `@page :first { size }`, именные страницы).
    pub(crate) geoms: std::cell::RefCell<Vec<PageGeom>>,
    /// Ячейка сетки листов — наибольший лист.
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
    /// Слой `position: fixed` — по копии на страницу БЕЗ сдвига: содержащий
    /// блок фиксированного — page area каждого листа (Blink `IsMonolithic`:
    /// «IsFixedPositioned() && GetDocument().Printing()» — монолит, повторяемый
    /// на каждой странице; `fixedpos-007..009`).
    pub(crate) fixed: Vec<Vec<AnyElement>>,
    pub(crate) plan: std::cell::RefCell<Vec<Frag>>,
    pub(crate) pages: std::cell::Cell<usize>,
    /// Листов в ряду и масштаб стопки.
    pub(crate) grid: std::cell::Cell<(usize, f32)>,
    /// Показываемые листы (с нуля, по возрастанию), `None` — все. Печатный
    /// reftest WPT сравнивает только страницы из `<meta name=reftest-pages>`.
    pub(crate) select: Option<Vec<usize>>,
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
            kids,
            geom_for,
            geoms: std::cell::RefCell::new(Vec::new()),
            cell: std::cell::Cell::new((1.0, 1.0)),
            margin_for,
            margin_els: Vec::new(),
            icb,
            icb_reach,
            fixed,
            plan: std::cell::RefCell::new(Vec::new()),
            pages: std::cell::Cell::new(1),
            grid: std::cell::Cell::new((1, 1.0)),
            select: None,
        }
    }

    /// Показать только листы `select` (номера с нуля).
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

impl Element for PageStack {
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
        // Стопка занимает РОДИТЕЛЯ целиком (окно стенда): число страниц
        // становится известно только после меры детей в `prepaint`, а размер
        // листов подгоняется масштабом, не размером стопки. Измеряемая
        // раскладка здесь не годится: блочный родитель не отдаёт ей
        // определённой высоты, и стопка получала 800x0 — масштаб схлопывался
        // в точку, кадр выходил пустым (первый заход, 06.09).
        let mut style = gpui::Style::default();
        style.size.width = gpui::relative(1.0).into();
        style.size.height = gpui::relative(1.0).into();
        let id = window.request_layout(style, [], cx);
        (id, ())
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
        // Первый лист — по имени первой коробки (css-page-3 §using-named-pages,
        // п. 3: «the first page … is given the start page value of the root»):
        // его page area — начальный содержащий блок и ширина меры.
        let first_name = self
            .kids
            .first()
            .map(|k| k.page.clone())
            .unwrap_or_default();
        let g = (self.geom_for)(0, &first_name);
        let (aw, ah) = (g.area.0.max(1.0), g.area.1.max(1.0));
        // 1. Мера: ширина — page area, высота — по содержимому. Поля детей
        //    — из меры (`PageKid::mt/mb`), у измеренных без меры они внутри обёртки.
        let kids: Vec<Kid> = self
            .kids
            .iter_mut()
            .map(|k| {
                let sz = k.el.layout_as_root(
                    size(
                        gpui::AvailableSpace::Definite(px(aw)),
                        gpui::AvailableSpace::MaxContent,
                    ),
                    window,
                    cx,
                );
                // Мера поддерева даёт точки разреза внутри ребёнка; без неё
                // ребёнок — цельный кусок измеренной высоты.
                let (h, cuts, forced, solid) = match &k.shape {
                    Some((h, cuts, forced, solid)) => {
                        (*h, cuts.clone(), forced.clone(), solid.clone())
                    }
                    None => (f32::from(sz.height), Vec::new(), Vec::new(), Vec::new()),
                };
                Kid {
                    h,
                    mt: k.mt,
                    mb: k.mb,
                    monolith: k.monolith,
                    cuts,
                    force_before: k.force_before,
                    force_after: k.force_after,
                    // Правило 1 §4.3 в ПЕЧАТИ пока не применяется (§2 отчёта):
                    // `PageKid` запретов не носит, а `false` в обоих полях
                    // включает быстрый выход `fill_avoiding` — путь страниц
                    // остаётся байт-в-байт прежним.
                    avoid_before: false,
                    avoid_after: false,
                    forced,
                    solid,
                    span: false,
                    // Страницы параллельный поток пока не берут: у них своё
                    // правило переполнения монолита (`fill_at`, ветка
                    // `paged && placed && cur > target`, crbug 1402540), и
                    // мешать их без отдельного замера печатного среза нельзя.
                    // `over == h` — поток выключен.
                    over: h,
                    // Страницы: `box-decoration-break` пока `slice` — весь
                    // кластер `clone` в колонках.
                    clone_dec: None,
                    // У страниц своё правило переполнения монолита (`fill_at`,
                    // ветка `paged && placed && cur > target`, crbug 1402540).
                    overflow_top: false,
                    repeat: RepeatGeom::default(),
                    par: Par::default(),
                }
            })
            .collect();
        let limit = self
            .kids
            .iter()
            .map(|k| k.frags.len() + 1)
            .min()
            .unwrap_or(1);
        // Листы бывают РАЗНЫЕ (`@page :first { size }`, `@page <имя>`), и
        // высота page area решает разрезы, а имя листа — от первой коробки на
        // нём, то есть от тех же разрезов. Укладка повторяется, пока имена
        // листов не перестанут меняться (у одинаковых листов — один проход,
        // байт-в-байт прежний).
        let kid_names: Vec<String> = self.kids.iter().map(|k| k.page.clone()).collect();
        let names_of = |plan: &[Frag], pages: usize| -> Vec<String> {
            let mut out: Vec<Option<String>> = vec![None; pages];
            for f in plan {
                if let Some(slot) = out.get_mut(f.col)
                    && slot.is_none()
                {
                    *slot = Some(kid_names[f.kid].clone());
                }
            }
            let mut last = first_name.clone();
            out.into_iter()
                .map(|n| {
                    if let Some(n) = n {
                        last = n;
                    }
                    last.clone()
                })
                .collect()
        };
        let mut geoms: Vec<PageGeom> = vec![g];
        let mut names: Vec<String> = vec![first_name.clone()];
        let mut result = None;
        for _ in 0..4 {
            let tail = names.last().cloned().unwrap_or_default();
            let at = |c: usize| -> f32 {
                geoms
                    .get(c)
                    .copied()
                    .unwrap_or_else(|| (self.geom_for)(c, &tail))
                    .area
                    .1
                    .max(1.0)
            };
            let (pages, _, plan) = ColumnStack::fill_at(&kids, &at, limit, true);
            let new_names = names_of(&plan, pages);
            let stable = new_names == names;
            names = new_names;
            geoms = names
                .iter()
                .enumerate()
                .map(|(i, n)| (self.geom_for)(i, n))
                .collect();
            result = Some((pages, plan));
            if stable {
                break;
            }
        }
        let (pages, plan) = result.unwrap_or_default();
        // Абсолюты корня добавляют листы, пока не кончится их досягаемость
        // (Blink: «If overflowed by monolithic overflow, we need more pages»,
        // box_fragment_builder.h). Потолок — число копий слоя.
        let icb_pages = if self.icb.is_empty() {
            0
        } else {
            ((self.icb_reach / ah).ceil().max(0.0) as usize).min(self.icb.len())
        };
        let pages = pages.max(icb_pages).max(1);
        // Листы сверх плана (досягаемость абсолютов) — с именем последнего.
        let tail = names.last().cloned().unwrap_or_default();
        while geoms.len() < pages {
            let i = geoms.len();
            geoms.push((self.geom_for)(i, &tail));
        }
        geoms.truncate(pages);
        // 2. Сетка листов и масштаб до вмещения в коробку стопки.
        let (ww, wh) = (
            f32::from(bounds.size.width).max(1.0),
            f32::from(bounds.size.height).max(1.0),
        );
        // Ячейка сетки — наибольший лист (у одинаковых — сам лист).
        let (pw, ph) = geoms
            .iter()
            .enumerate()
            .filter(|(i, _)| self.slot(*i).is_some())
            .fold((1.0f32, 1.0f32), |(w, h), (_, g)| {
                (w.max(g.shown().0), h.max(g.shown().1))
            });
        self.cell.set((pw, ph));
        let shown = (0..pages)
            .filter(|&i| self.slot(i).is_some())
            .count()
            .max(1);
        let mut best = (1usize, 0.0f32);
        for per_row in 1..=shown {
            let rows = shown.div_ceil(per_row);
            let s = (ww / (per_row as f32 * pw))
                .min(wh / (rows as f32 * ph))
                .min(1.0);
            if s > best.1 {
                best = (per_row, s);
            }
        }
        self.grid.set(best);
        self.pages.set(pages);
        // 3. Копии раскладываются ЦЕЛИКОМ и поднимаются на срез — ровно как
        //    в `ColumnStack::prepaint`; видимую часть делает маска. Ширина —
        //    page area СВОЕГО листа (`page-size-004`: `width: 50%` на листе
        //    100px — 50, на листах 320px — 160).
        for f in &plan {
            let (sx, sy) = self.sheet_origin(f.col);
            let pg = geoms.get(f.col).copied().unwrap_or(g);
            let (ax, ay) = pg.area_origin();
            let aw = pg.area.0.max(1.0);
            let full_h = kids[f.kid].h;
            let kid = &mut self.kids[f.kid];
            let inner_top = kid.inner_top;
            let el = if f.copy == 0 {
                &mut kid.el
            } else {
                match kid.frags.get_mut(f.copy - 1) {
                    Some(e) => e,
                    None => continue,
                }
            };
            el.layout_as_root(
                size(
                    gpui::AvailableSpace::Definite(px(aw)),
                    gpui::AvailableSpace::Definite(px(full_h)),
                ),
                window,
                cx,
            );
            el.prepaint_at(
                point(
                    bounds.origin.x + px(sx + ax),
                    bounds.origin.y + px(sy + ay + f.y - f.from - inner_top),
                ),
                window,
                cx,
            );
        }
        // Слой ICB — копия `p` на листе `p`, в его page area, поднятая на `p`
        // высот area: непрерывный поток абсолютов, разрезанный страницами.
        for p in 0..pages.min(self.icb.len()) {
            let (sx, sy) = self.sheet_origin(p);
            let (ax, ay) = geoms[p].area_origin();
            let lift = p as f32 * ah;
            for el in &mut self.icb[p] {
                el.layout_as_root(
                    size(
                        gpui::AvailableSpace::Definite(px(aw)),
                        gpui::AvailableSpace::Definite(px(ah)),
                    ),
                    window,
                    cx,
                );
                el.prepaint_at(
                    point(
                        bounds.origin.x + px(sx + ax),
                        bounds.origin.y + px(sy + ay - lift),
                    ),
                    window,
                    cx,
                );
            }
        }
        // Слой `fixed` — копия `p` на листе `p` без сдвига: page area каждого
        // листа — его содержащий блок.
        for p in 0..pages.min(self.fixed.len()) {
            let (sx, sy) = self.sheet_origin(p);
            // Размер содержащего блока — page area ПЕРВОГО листа: Blink
            // раскладывает `fixed` один раз от начального содержащего блока и
            // повторяет на каждом листе (`fixedpos-010-print`: `right: -100px`
            // при листе 400 — за краем, на листах 500 — в правом нижнем углу).
            let (ax, ay) = geoms[p].area_origin();
            for el in &mut self.fixed[p] {
                el.layout_as_root(
                    size(
                        gpui::AvailableSpace::Definite(px(aw)),
                        gpui::AvailableSpace::Definite(px(ah)),
                    ),
                    window,
                    cx,
                );
                el.prepaint_at(
                    point(bounds.origin.x + px(sx + ax), bounds.origin.y + px(sy + ay)),
                    window,
                    cx,
                );
            }
        }
        // Марджин-боксы — после листов: `counter(pages)` знает их число.
        self.margin_els.clear();
        if let Some(mf) = self.margin_for.clone() {
            for (p, pg) in geoms.iter().enumerate() {
                let boxes = mf(
                    p,
                    &names.get(p).cloned().unwrap_or_else(|| tail.clone()),
                    pages,
                    pg,
                );
                let (sx, sy) = self.sheet_origin(p);
                for (rect, mut el) in layout_margin_boxes(boxes, pg, window, cx) {
                    el.layout_as_root(
                        size(
                            gpui::AvailableSpace::Definite(px(rect.2.max(0.0))),
                            gpui::AvailableSpace::Definite(px(rect.3.max(0.0))),
                        ),
                        window,
                        cx,
                    );
                    el.prepaint_at(
                        point(
                            bounds.origin.x + px(sx + rect.0),
                            bounds.origin.y + px(sy + rect.1),
                        ),
                        window,
                        cx,
                    );
                    self.margin_els.push((p, el));
                }
            }
        }
        *self.plan.borrow_mut() = plan;
        *self.geoms.borrow_mut() = geoms;
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
        let s = self.grid.get().1;
        let pages = self.pages.get();
        // Масштаб вокруг левого верха стопки; матрица — в точках устройства,
        // как у `interact::Transformed` (interact.rs:1152-1188).
        let k = window.scale_factor();
        let dev = |v: f32| px(v).scale(k);
        let origin = point(
            dev(f32::from(bounds.origin.x)),
            dev(f32::from(bounds.origin.y)),
        );
        let back = point(
            dev(-f32::from(bounds.origin.x)),
            dev(-f32::from(bounds.origin.y)),
        );
        let matrix = gpui::TransformationMatrix::unit()
            .translate(origin)
            .compose(gpui::TransformationMatrix {
                rotation_scale: [[s, 0.0], [0.0, s]],
                translation: [0.0, 0.0],
            })
            .translate(back);
        let plan = self.plan.borrow().clone();
        let masks: Vec<gpui::ContentMask<Pixels>> =
            (0..pages).map(|i| self.area_mask(bounds, i)).collect();
        let rect = |x: f32, y: f32, w: f32, h: f32| Bounds {
            origin: point(bounds.origin.x + px(x), bounds.origin.y + px(y)),
            size: size(px(w), px(h)),
        };
        window.with_transformation(matrix, |window| {
            // Порядок краски css-page-3 §painting: фон листа → канвас
            // документа (border box листа) → рамки → содержимое.
            for i in 0..pages {
                if self.slot(i).is_none() || self.geom(i).turn % 2 == 1 {
                    continue;
                }
                let (sx, sy) = self.sheet_origin(i);
                let g = self.geom(i);
                window.paint_quad(gpui::fill(rect(sx, sy, g.size.0, g.size.1), g.bg));
                let bx = sx + g.margin[3];
                let by = sy + g.margin[0];
                let bw = (g.size.0 - g.margin[1] - g.margin[3]).max(0.0);
                let bh = (g.size.1 - g.margin[0] - g.margin[2]).max(0.0);
                if let Some(c) = g.canvas {
                    // Канвас кроет border box листа (§painting), но не шире
                    // самого листа: при отрицательных полях жёлтый фон тела
                    // вылезал полосами за правый и нижний край.
                    let (cx, cy) = (bx.max(sx), by.max(sy));
                    let cw = ((bx + bw).min(sx + g.size.0) - cx).max(0.0);
                    let ch = ((by + bh).min(sy + g.size.1) - cy).max(0.0);
                    window.paint_quad(gpui::fill(rect(cx, cy, cw, ch), c));
                }
                let (t, c) = g.border;
                if t > 0.0 {
                    for r in [
                        (bx, by, bw, t),
                        (bx, by + bh - t, bw, t),
                        (bx, by, t, bh),
                        (bx + bw - t, by, t, bh),
                    ] {
                        window.paint_quad(gpui::fill(rect(r.0, r.1, r.2, r.3), c));
                    }
                }
                // Контур — снаружи рамки со сдвигом, в слое рамок (под
                // содержимым): `page-box-010` — поле 50, контур 10 со сдвигом
                // 40 ложится вплотную к краю листа, как рамка эталона.
                let (ow, off, oc) = g.outline;
                if ow > 0.0 {
                    let (ox, oy) = (bx - off - ow, by - off - ow);
                    let (fw, fh) = (bw + 2.0 * (off + ow), bh + 2.0 * (off + ow));
                    for r in [
                        (ox, oy, fw, ow),
                        (ox, oy + fh - ow, fw, ow),
                        (ox, oy, ow, fh),
                        (ox + fw - ow, oy, ow, fh),
                    ] {
                        window.paint_quad(gpui::fill(rect(r.0, r.1, r.2, r.3), oc));
                    }
                }
            }
            // Содержимое — под маской СВОЕГО ФРАГМЕНТА: копия нарисована во
            // всю высоту, видна только полоса `[y, y + h)` этой страницы (вид
            // `slice`, css-break-4 §4; ровно как у `ColumnStack`). Маска по
            // целой page area оставляла на странице хвост следующего
            // фрагмента до края листа (`block-page-break-inside-avoid-7`).
            for f in plan.iter().copied() {
                if self.geom(f.col).turn % 2 == 1 {
                    continue;
                }
                let Some(page) = masks.get(f.col).cloned() else {
                    continue;
                };
                let (sx, sy) = self.sheet_origin(f.col);
                let g = self.geom(f.col);
                let (ax, ay) = g.area_origin();
                let mask = gpui::ContentMask {
                    bounds: Bounds {
                        origin: point(
                            bounds.origin.x + px((sx + ax) * s),
                            bounds.origin.y + px((sy + ay + f.y) * s),
                        ),
                        size: size(px(g.area.0 * s), px(f.h * s)),
                    }
                    .intersect(&page.bounds),
                };
                let kid = &mut self.kids[f.kid];
                let el = if f.copy == 0 {
                    &mut kid.el
                } else {
                    match kid.frags.get_mut(f.copy - 1) {
                        Some(e) => e,
                        None => continue,
                    }
                };
                // Маски детей (overflow ячеек, полосы таблиц, срезы) заданы в
                // немасштабированных точках — под матрицей стопки они обязаны
                // пройти то же подобие (`Window::with_mask_scale`).
                window.with_content_mask(Some(mask), |window| {
                    window.with_mask_scale(bounds.origin, s, |window| el.paint(window, cx))
                });
            }
            for p in 0..pages.min(self.icb.len()) {
                if self.geom(p).turn % 2 == 1 {
                    continue;
                }
                let Some(mask) = masks.get(p).cloned() else {
                    continue;
                };
                for el in &mut self.icb[p] {
                    window.with_content_mask(Some(mask), |window| {
                        window.with_mask_scale(bounds.origin, s, |window| el.paint(window, cx))
                    });
                }
            }
            for p in 0..pages.min(self.fixed.len()) {
                if self.geom(p).turn % 2 == 1 {
                    continue;
                }
                let Some(mask) = masks.get(p).cloned() else {
                    continue;
                };
                for el in &mut self.fixed[p] {
                    window.with_content_mask(Some(mask), |window| {
                        window.with_mask_scale(bounds.origin, s, |window| el.paint(window, cx))
                    });
                }
            }
            // Марджин-боксы — ПОСЛЕДНИМ слоем (css-page-3 §painting: «page-margin
            // boxes» после содержимого документа), под маской всего листа.
            let sheets: Vec<Bounds<Pixels>> = (0..pages)
                .map(|p| {
                    let (sx, sy) = self.sheet_origin(p);
                    let g = self.geom(p);
                    Bounds {
                        origin: point(bounds.origin.x + px(sx * s), bounds.origin.y + px(sy * s)),
                        size: size(px(g.size.0 * s), px(g.size.1 * s)),
                    }
                })
                .collect();
            for (p, el) in &mut self.margin_els {
                if self.geoms.borrow().get(*p).is_some_and(|g| g.turn % 2 == 1) {
                    continue;
                }
                let Some(sheet) = sheets.get(*p).copied() else {
                    continue;
                };
                let mask = gpui::ContentMask { bounds: sheet };
                window.with_content_mask(Some(mask), |window| {
                    window.with_mask_scale(bounds.origin, s, |window| el.paint(window, cx))
                });
            }
        });
        // Повёрнутые листы (`page-orientation`): тот же порядок слоёв, но под
        // СВОЕЙ матрицей — поворот на четверть оборота вокруг листа, затем
        // масштаб стопки. Лист разложен в ячейке как неповёрнутый (левый верх
        // `O`); маски детей и фрагментов заданы в его непреобразованных
        // точках и едут вместе с ним (`with_transformation_masked`: поворот на
        // четверть оборота оси сохраняет).
        for i in 0..pages {
            let g = self.geom(i);
            if self.slot(i).is_none() || g.turn.is_multiple_of(2) {
                continue;
            }
            let (sx, sy) = self.sheet_origin(i);
            let (bx0, by0) = (
                f32::from(bounds.origin.x) * k,
                f32::from(bounds.origin.y) * k,
            );
            let (ox, oy) = (bx0 + sx * k, by0 + sy * k);
            let (w, h) = (g.size.0 * k, g.size.1 * k);
            // p -> O + R(p - O) + сдвиг, затем q -> B + s(q - B).
            let (rs, t) = if g.turn == 1 {
                // rotate-right: (x, y) -> (O.x + H - (y - O.y), O.y + (x - O.x)).
                (
                    [[0.0, -s], [s, 0.0]],
                    [
                        bx0 * (1.0 - s) + s * (ox + h + oy),
                        by0 * (1.0 - s) + s * (oy - ox),
                    ],
                )
            } else {
                // rotate-left: (x, y) -> (O.x + (y - O.y), O.y + W - (x - O.x)).
                (
                    [[0.0, s], [-s, 0.0]],
                    [
                        bx0 * (1.0 - s) + s * (ox - oy),
                        by0 * (1.0 - s) + s * (oy + w + ox),
                    ],
                )
            };
            let m = gpui::TransformationMatrix {
                rotation_scale: rs,
                translation: t,
            };
            let plan_i: Vec<Frag> = plan.iter().copied().filter(|f| f.col == i).collect();
            window.with_transformation_masked(m, |window| {
                window.paint_quad(gpui::fill(rect(sx, sy, g.size.0, g.size.1), g.bg));
                let bx = sx + g.margin[3];
                let by = sy + g.margin[0];
                let bw = (g.size.0 - g.margin[1] - g.margin[3]).max(0.0);
                let bh = (g.size.1 - g.margin[0] - g.margin[2]).max(0.0);
                if let Some(c) = g.canvas {
                    let (cx0, cy0) = (bx.max(sx), by.max(sy));
                    let cw = ((bx + bw).min(sx + g.size.0) - cx0).max(0.0);
                    let ch = ((by + bh).min(sy + g.size.1) - cy0).max(0.0);
                    window.paint_quad(gpui::fill(rect(cx0, cy0, cw, ch), c));
                }
                let (bt, bc) = g.border;
                if bt > 0.0 {
                    for r in [
                        (bx, by, bw, bt),
                        (bx, by + bh - bt, bw, bt),
                        (bx, by, bt, bh),
                        (bx + bw - bt, by, bt, bh),
                    ] {
                        window.paint_quad(gpui::fill(rect(r.0, r.1, r.2, r.3), bc));
                    }
                }
                let (ax, ay) = g.area_origin();
                let area = rect(sx + ax, sy + ay, g.area.0, g.area.1);
                for f in plan_i {
                    let mask = gpui::ContentMask {
                        bounds: rect(sx + ax, sy + ay + f.y, g.area.0, f.h).intersect(&area),
                    };
                    let kid = &mut self.kids[f.kid];
                    let el = if f.copy == 0 {
                        &mut kid.el
                    } else {
                        match kid.frags.get_mut(f.copy - 1) {
                            Some(e) => e,
                            None => continue,
                        }
                    };
                    window.with_content_mask(Some(mask), |window| el.paint(window, cx));
                }
                if let Some(layer) = self.icb.get_mut(i) {
                    for el in layer {
                        window.with_content_mask(
                            Some(gpui::ContentMask { bounds: area }),
                            |window| el.paint(window, cx),
                        );
                    }
                }
                if let Some(layer) = self.fixed.get_mut(i) {
                    for el in layer {
                        window.with_content_mask(
                            Some(gpui::ContentMask { bounds: area }),
                            |window| el.paint(window, cx),
                        );
                    }
                }
                let sheet = rect(sx, sy, g.size.0, g.size.1);
                for (p, el) in &mut self.margin_els {
                    if *p == i {
                        window.with_content_mask(
                            Some(gpui::ContentMask { bounds: sheet }),
                            |window| el.paint(window, cx),
                        );
                    }
                }
            });
        }
    }
}

impl IntoElement for PageStack {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
