//! Фрагментация: формы, полосы, переносимые коробки.
// owner: A

use crate::render::*;

pub mod breaks;
pub mod clone;
pub mod flex_lines;
pub mod grid_bands;
pub mod line_shape;
pub mod probe;
pub mod push;
pub mod shape_contents;
pub mod table_bands;
pub mod types;

/// Элемент сетки для `grid_auto_row_bands`: номер среди `c.children`, ряд,
/// верхнее поле, мера `shape_full`, годны ли его внутренние точки сетке.
pub(crate) type GridSpot = (usize, usize, f32, Shape, bool);

/// То же плюс смещения принудительных разрывов и диапазоны
/// монолитов внутри.
pub(crate) type Shape = (f32, f32, f32, Vec<(f32, f32)>, Vec<f32>, Vec<(f32, f32)>);

/// Условия меры: `paged` — стопка страниц (монолитом считается и
/// `contain: size`, см. `solid_box`); `viewport` — размер области просмотра
/// для `vh`/`vw`: у страниц это page area, у колонок единицы окна остаются
/// неразрешёнными (`None` → отказ от меры, прежнее поведение).
#[derive(Clone, Copy)]
pub(crate) struct ShapeCx {
    pub(crate) paged: bool,
    pub(crate) viewport: Option<(f32, f32)>,
    /// Презентационный `cellpadding` таблицы — отступ ЭТОЙ ячейки поверх
    /// умолчания `td { padding: 1px }` (как в `table()`); потомкам не
    /// передаётся.
    pub(crate) cell_pad: Option<f32>,
    /// Мера ПОТОКА, а не коробки: заданная высота не обрезает ни высоту, ни
    /// точки разреза, ни монолитные диапазоны. Переполнение коробки с
    /// заданной высотой — параллельный поток (css-break-3 §3), и укладке
    /// колонок нужна его протяжённость ОТДЕЛЬНО от высоты коробки.
    /// Бюджет ОДИН на путь: флаг гаснет на первой же коробке с заданной
    /// высотой (`shape_full`, перепривязка `cx`). Вложенная ограниченная
    /// коробка заводит СВОЙ параллельный поток, в поток предка он не
    /// входит, а модель несёт один `over` на ребёнка стопки — второго
    /// потока ей выразить нечем. Через коробки с высотой `auto` флаг идёт
    /// насквозь: там своей ограниченности нет
    /// (`overflowed-block-with-room-after-003` — обёртка `auto` над
    /// коробкой 70).
    pub(crate) unclamped: bool,
}

impl ShapeCx {
    pub(crate) const COLUMNS: ShapeCx = ShapeCx {
        paged: false,
        viewport: None,
        cell_pad: None,
        unclamped: false,
    };
}

/// Кадр меры строк: наследованный стиль коробки и ширина её содержимого
/// (`None` — неизвестна, строки не меряются).
pub(crate) struct LineFrame {
    pub(crate) inh: Computed,
    pub(crate) w: Option<f32>,
    /// Анонимный блок строк хоста (`group_inline_runs`): его срез — срез хоста.
    pub(crate) anon: bool,
    /// Ширина детей-элементов этой коробки (`items_kind`): 0 — блочный поток,
    /// 1 — растянутые на всю ширину (колонка flex и сетка-стопка при
    /// `stretch`), 2 — ширина по раскладке (ряд flex, прочая сетка): известна
    /// лишь заданная в точках.
    pub(crate) items: u8,
}

/// Контекст меры строк для `shape_full`: включается только вокруг меры детей
/// стопки колонок (`with_lines`), где ширина колонки известна. `shape_full` о
/// наследовании и ширине ничего не знает (ей дают голый элемент), поэтому
/// кадры ведёт `LineScope` на входе в неё.
pub(crate) struct LineCx {
    pub(crate) opts: RenderOpts,
    pub(crate) frames: Vec<LineFrame>,
}

thread_local! {
    pub(crate) static LINE_CX: std::cell::RefCell<Option<LineCx>> = const { std::cell::RefCell::new(None) };
}

/// Выполнить `f` с контекстом меры строк: `base` — стиль многоколоночника,
/// `w` — строчный размер колонки.
pub(crate) fn with_lines<T>(base: &Computed, w: Option<f32>, opts: &RenderOpts, f: impl FnOnce() -> T) -> T {
    let prev = LINE_CX.with(|l| {
        l.borrow_mut().replace(LineCx {
            opts: opts.clone(),
            frames: vec![LineFrame { inh: base.clone(), w, anon: false, items: 0 }],
        })
    });
    let out = f();
    LINE_CX.with(|l| *l.borrow_mut() = prev);
    out
}

/// Кадр меры строк на время `shape_full(c)`.
pub(crate) struct LineScope(pub(crate) bool);

impl LineScope {
    pub(crate) fn enter(c: &Element) -> Self {
        LINE_CX.with(|l| {
            let mut g = l.borrow_mut();
            let Some(cx) = g.as_mut() else {
                return LineScope(false);
            };
            let Some(top) = cx.frames.last() else {
                return LineScope(false);
            };
            let inh = crate::inline::inherit(&top.inh, &c.style);
            let w = top.w.and_then(|pw| match top.items {
                // Элемент flex/сетки шириной по раскладке: известна лишь
                // заданная в точках.
                2 if !matches!(c.style.width, Some(Len::Px(_))) => None,
                // Растянутый элемент (css-flexbox-1 §9.4 шаг 11 / css-grid-2
                // §11.3 `stretch`): ширина как у блока в потоке, если сам
                // элемент выравнивание не переопределил.
                1 if c.style.align_self.is_some() || c.style.justify_self.is_some() => None,
                _ => line_content_w(c, pw),
            });
            cx.frames.push(LineFrame { inh, w, anon: c.tag == "anon-block", items: items_kind(c) });
            LineScope(true)
        })
    }
}

impl Drop for LineScope {
    fn drop(&mut self) {
        if self.0 {
            LINE_CX.with(|l| {
                if let Some(cx) = l.borrow_mut().as_mut() {
                    cx.frames.pop();
                }
            });
        }
    }
}
