//! Сборка дерева узлов в элементы GPUI.
//!
//! Блочные узлы становятся `div` со своим стилем; подряд идущие инлайн-узлы
//! собираются в один абзац (`inline.rs`). Списки, таблицы и картинки имеют
//! свои правила — они и описаны в доке отдельными разделами.

mod paragraph_route;
use paragraph_route::paint_inline_step7;
use paragraph_route::paragraph_probed;

mod paint_order;
pub(super) use paint_order::inline_abs_paint_last;
use paint_order::paint_last_ok;
use paint_order::unkeyed_positions;

use crate::dom::Node;
use crate::layout::block::containing::AVAIL_W;
use crate::layout::float::clear::without_inert_clear;
use crate::layout::replaced::iframe::IFRAME_DEPTH;
use crate::paint::effects::mask::collect_mask_defs;
use crate::style::computed::Computed;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, TextStyle, div, px};
pub(super) mod content_wrapper;
mod first_line_text;
pub(crate) use content_wrapper::{content_sized, content_sized_wraps};
mod first_letter_descendants;
mod first_letter_scope;
mod first_line_descendants;
mod inline_splits;
pub(super) mod pseudo_line_layers;
pub(crate) use inline_splits::split_block_in_inline;
pub(super) mod native_paragraph_route;

pub(super) mod box_div;
pub(crate) use crate::render::box_div::*;
pub(super) mod classify;
pub(crate) use crate::render::classify::*;
pub(super) mod util;
pub use crate::layout::page::names::{PageMarginDecls, PageMarginDeclsFn, first_page_name};
pub use crate::layout::page::paged::{render_paged, render_paged_select};
pub(crate) use crate::render::util::*;
pub(crate) mod paragraph;
pub(crate) use crate::render::paragraph::atom_piece::*;
pub(crate) use crate::render::paragraph::pieces::*;
pub(crate) use crate::render::paragraph::*;
pub(crate) mod blocks;
pub(crate) use crate::render::blocks::*;
pub(super) mod element;
pub(super) mod generic_box;
pub(crate) use crate::render::element::*;
use crate::render::generic_box::*;

/// Настройки отрисовки: то, что задаёт приложение, а не документ.
#[derive(Clone)]
pub struct RenderOpts {
    /// Базовый стиль текста — от него считаются прогоны и наследование.
    pub text: TextStyle,
    /// Размер окна в точках — от него считаются `vh` и `vw`.
    pub viewport: (f32, f32),
    /// Множитель строки при `line-height: normal`.
    ///
    /// Браузер берёт его из метрик шрифта — у интерфейсных это около 1.31
    /// кегля. Умолчание GPUI — золотое сечение (1.618), и без своего значения
    /// КАЖДЫЙ блок текста выходил на четверть выше браузерного, а разница
    /// копилась вниз по документу.
    pub normal_line_height: f32,
    /// Соль документа для буферов проб (`Document::key`).
    ///
    /// Номера узлов считаются с нуля в каждом документе: когда в одном
    /// потоке живут два документа сразу (стенд гонит пары параллельно),
    /// полоса фона одного забирала прямоугольники ячеек другого с тем же
    /// номером узла. Ноль допустим, пока документ один.
    pub doc_salt: u64,
}

impl RenderOpts {
    /// Цвет подложки выделения.
    ///
    /// Отдельного поля в настройках нет, чтобы не ломать вызывающих: берём
    /// цвет текста и делаем из него полупрозрачную подложку — она читается
    /// и на светлой, и на тёмной теме.
    pub(crate) fn selection_color(&self) -> gpui::Hsla {
        let mut c = self.text.color;
        c.a = 0.25;
        c
    }

    pub(crate) fn base_size(&self) -> f32 {
        f32::from(self.text.font_size.to_pixels(px(16.)))
    }

    /// Корневой стиль документа.
    ///
    /// Высота строки тут НЕ задаётся: `normal` по CSS — метрика шрифта, и
    /// считает её `normal_fraction` по семейству элемента. Пока корень
    /// навязывал постоянную долю, она наследовалась ВСЕМ, и замер шрифта не
    /// работал ни разу: коробка с `line-height: normal` выходила выше коробки
    /// с `line-height: 1em` при одном и том же шрифте.
    pub(crate) fn root_style(&self) -> Computed {
        Computed::default()
    }
}

pub fn render(nodes: &[Node], opts: &RenderOpts) -> Vec<AnyElement> {
    let stripped = without_inert_clear(nodes);
    let nodes: &[Node] = stripped.as_deref().unwrap_or(nodes);
    crate::text::metrics::set_doc_family(&opts.text.font_family);
    let root = opts.root_style();
    crate::interactive::frame::frame_sanitize();
    // Пойманная паника кадра внутри рамки оставляла счётчик глубины
    // навсегда — три такие паники, и рамки исчезали до перезапуска.
    IFRAME_DEPTH.with(|d| d.set(0));
    collect_mask_defs(nodes);
    // Слой начального содержащего блока: внепоточные элементы без
    // позиционированного предка дописываются последними детьми документа —
    // их края решает область просмотра (§10.1 п.4).
    crate::layout::positioned::containing_block::icb_open();
    // Корень документа получает ширину области просмотра: от неё цепочка
    // `AVAIL_W` вычитает поля/рамки/отступы `html` и `body`.
    let avail_prev = AVAIL_W.replace(Some(opts.viewport.0).filter(|w| *w > 0.0));
    // Шаг 8 приложения E: позиционированные `z-index: auto | 0` красятся
    // после потока корневого контекста в порядке разметки — собиратель
    // `gpui::PaintCollect` между парой меток. Внешняя сборка идёт вне краски
    // и сбрасывает собиратели (пойманная паника кадра оставила бы их
    // открытыми); вложенный документ (рамка) собирает своё внутри.
    let depth = RENDER_DEPTH.with(|d| {
        d.set(d.get() + 1);
        d.get()
    });
    if depth == 1 {
        gpui::paint_collect_reset();
    }
    let unkeyed_prev = UNKEYED.replace(unkeyed_positions(nodes));
    let mut out = blocks(nodes, &root, opts);
    AVAIL_W.set(avail_prev);
    out.extend(crate::layout::positioned::containing_block::icb_close());
    UNKEYED.replace(unkeyed_prev);
    RENDER_DEPTH.with(|d| d.set(d.get() - 1));
    let (open, close) = gpui::PaintCollect::pair();
    out.insert(0, open.into_any_element());
    out.push(close.into_any_element());
    out
}

thread_local! {
    /// Глубина вложенных `render` (документ в рамке собирается внутри).
    static RENDER_DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    /// Номер элемента в порядке сборки — ключ краски шага 8 (`PaintLast`).
    static PAINT_KEY: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    /// Позиции узлов в прямом обходе документа и позиция ПОСЛЕДНЕГО
    /// позиционированного, который ключа краски не получает (см.
    /// `unkeyed_positions`).
    static UNKEYED: std::cell::RefCell<(std::collections::HashMap<u64, usize>, Option<usize>)> =
        std::cell::RefCell::new((std::collections::HashMap::new(), None));
}

/// Следующий ключ краски: зовётся при входе в элемент, до сборки детей, —
/// предок получает ключ меньше потомков (прямой обход).
fn next_paint_key() -> u64 {
    PAINT_KEY.with(|k| {
        let v = k.get().wrapping_add(1);
        k.set(v);
        v
    })
}

/// Один блок верхнего уровня — единица виртуализации.
///
/// Список GPUI спрашивает только видимые блоки, и невидимая часть документа
/// не стоит ничего: ни раскладки, ни отрисовки. Это то же ухищрение, которым
/// держится дерево файлов и чат.
pub fn render_block(nodes: &[Node], index: usize, opts: &RenderOpts) -> Option<AnyElement> {
    let node = nodes.get(index)?;
    crate::interactive::frame::frame_sanitize();
    IFRAME_DEPTH.with(|d| d.set(0));
    collect_mask_defs(nodes);
    let root = opts.root_style();
    // Слой ICB закрывается на блок ленты: дальше своего блока внепоточный
    // элемент всё равно не уедет, а без слоя он остался бы на месте.
    crate::layout::positioned::containing_block::icb_open();
    let avail_prev = AVAIL_W.replace(Some(opts.viewport.0).filter(|w| *w > 0.0));
    let out = blocks(std::slice::from_ref(node), &root, opts);
    AVAIL_W.set(avail_prev);
    let layer = crate::layout::positioned::containing_block::icb_close();
    let first = out.into_iter().next()?;
    if layer.is_empty() {
        return Some(first);
    }
    // Лента отдаёт РОВНО ОДИН элемент на блок, поэтому слой уходит внутрь
    // обёртки. Содержащим блоком становится она, а не окно: в ленте окна
    // всё равно нет — блок живёт в прокрутке. Без обёртки вынесенный
    // элемент просто пропадал бы с экрана.
    Some(
        div()
            .relative()
            .child(first)
            .children(layer)
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests;
