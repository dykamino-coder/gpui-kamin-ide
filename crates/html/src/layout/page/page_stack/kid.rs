//! Ребёнок стопки страниц `PageKid`: блок верхнего уровня документа с копиями на разрез,
//! мерой поддерева, признаками монолита и принудительных разрывов, именем страницы и
//! повторяемыми полосами таблицы, которые красятся в своих масках на каждом листе.

use gpui::AnyElement;

impl PageKid {
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
    pub el: AnyElement,
    /// Recorder of the main copy: measured lines and, for a group of one block
    /// box, its block stack (`paged::kids::breaks`).
    pub(crate) watched: Option<std::rc::Rc<crate::layout::fragment::paginate::watch::Watched>>,
    /// A root float contributes its measured border box even outside normal flow.
    pub(crate) float_id: Option<u64>,
    /// Repeated table sections reserved by the common fragmentation planner.
    pub repeat: Option<crate::layout::fragment::types::Repeat>,
    /// Копии на случай разреза между страницами (см. `StackChild::frags`).
    pub frags: Vec<AnyElement>,
    /// Монолит (css-break-4 §4.1 плюс `contain: size`, как Blink `IsMonolithic`).
    pub monolith: bool,
    /// Принудительный разрыв страницы перед/после (css-break-4 §3.1), включая
    /// смену имени страницы (css-page-3 §"Using named pages", п. 4).
    pub force_before: bool,
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
