//! Ребёнок стопки страниц `PageKid`: блок верхнего уровня документа с копиями на разрез,
//! мерой поддерева, признаками монолита и принудительных разрывов, именем страницы и
//! повторяемыми полосами таблицы, которые красятся в своих масках на каждом листе.

use gpui::AnyElement;

pub(crate) struct LeafPaint {
    pub(crate) geometry: std::rc::Rc<
        dyn Fn(
            crate::layout::fragment::token::Space,
        ) -> crate::layout::fragment::flow::blocks::leaf::Leaf,
    >,
    pub(crate) build: std::rc::Rc<
        dyn Fn(
            crate::layout::fragment::token::Space,
            crate::layout::fragment::types::Frag,
        ) -> AnyElement,
    >,
}

impl PageKid {
    /// Break propagation follows measured flex line membership.
    pub(super) fn measured_edge(&self, last: bool) -> (bool, bool) {
        self.regrow
            .as_ref()
            .and_then(|r| {
                self.watched
                    .as_ref()?
                    .blocks()
                    .map(|b| b.edge(last, &r.laid))
            })
            .unwrap_or((
                if last {
                    self.force_after
                } else {
                    self.force_before
                },
                false,
            ))
    }

    /// Полосы повтора таблицы по её разложенным секциям `sections` на листе с
    /// page area высотой `per` (`table_bands::repeat_spec`); без повторяемой
    /// секции повтора нет вовсе.
    pub(super) fn refresh_repeat(
        &mut self,
        sections: Option<crate::layout::fragment::flow::blocks::table::TableSections>,
        per: f32,
    ) {
        let Some(sec) = sections else {
            return;
        };
        if self.repeat.is_none() {
            return;
        }
        match crate::layout::fragment::table_bands::repeat_spec(sec, per) {
            Some((head, foot, geom)) => {
                if let Some(r) = self.repeat.as_mut() {
                    r.head = head;
                    r.foot = foot;
                    r.geom = geom;
                }
            }
            None => self.repeat = None,
        }
    }

    pub(super) fn paint_sections(
        &mut self,
        f: &crate::layout::fragment::types::Frag,
        band: impl Fn(f32, f32) -> gpui::Bounds<gpui::Pixels>,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) {
        let Some(r) = self.repeat.as_mut() else {
            return;
        };
        r.paint_bands(f, |el, top, h| {
            window.with_content_mask(
                Some(gpui::ContentMask {
                    bounds: band(top, h),
                }),
                |window| el.paint(window, cx),
            );
        });
    }
}

/// Блок верхнего уровня документа в стопке страниц. Высота заранее НЕ
/// известна и меряется раскладкой (`layout_as_root`) в `prepaint` — так в
/// стопку попадает и голый текст, которого укладка колонок не видит.
// A page shape stores height, cuts, forced breaks and monolithic intervals together.
#[allow(clippy::type_complexity)]
pub struct PageKid {
    #[cfg(feature = "wpt-harness")]
    pub(crate) probe: crate::FragmentProbes,
    pub el: AnyElement,
    /// Повторная раскладка flex после роста независимого потока.
    pub(crate) regrow: Option<crate::layout::fragment::push::Regrow>,
    /// Исходные CB продолжаемой коробки до снятия корневых обёрток.
    pub(crate) percentages: Option<crate::layout::fragment::flow::PagePercentages>,
    /// Paint slices первого token consumer; создаются только по готовому плану.
    pub(crate) leaf: Option<LeafPaint>,
    /// Recorder of the main copy: measured lines and, for a group of one block
    /// box, its block stack (`paged::kids::breaks`).
    pub(crate) watched: Option<std::rc::Rc<crate::layout::fragment::paginate::watch::Watched>>,
    /// A root float contributes its measured border box even outside normal flow.
    pub(crate) float_id: Option<u64>,
    /// Корневые floats задают измеренную протяжённость за пределами normal flow.
    pub(crate) float_extent: bool,
    /// Repeated table sections reserved by the common fragmentation planner.
    pub repeat: Option<crate::layout::fragment::types::Repeat>,
    /// Копии на случай разреза между страницами (см. `StackChild::frags`).
    pub frags: Vec<AnyElement>,
    /// Монолит (css-break-4 §4.1 плюс `contain: size`, как Blink `IsMonolithic`).
    pub monolith: bool,
    /// Принудительный разрыв страницы перед/после (css-break-4 §3.1), включая
    /// смену имени страницы (css-page-3 §"Using named pages", п. 4).
    pub force_before: bool,
    /// Смена имени страницы создаёт собственный разрыв перед группой.
    pub(crate) renamed: bool,
    pub force_after: bool,
    /// Мера поддерева от `render::shape_full` (css-break-4 §possible-breaks):
    /// высота, точки разреза `(need, from)`, принудительные разрывы и монолиты
    /// — как у `StackChild`; `None` — высота измеренная, разрезов нет.
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
    /// Flow extent with visible overflow: reserves pages (Blink `ReserveSpaceForMonolithicOverflow`).
    pub reach: Option<f32>,
    /// Block size used by the plan (`prepaint`): fragments reaching it show overflow.
    pub end: std::cell::Cell<f32>,
}
