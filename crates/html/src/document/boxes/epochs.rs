//! Эпохи входов раскладки коробки: собственных, поддерева и вклада в intrinsic-размер.
//!
//! Значение эпохи — номер публикации (`BoxTree::clock`), на которой вход изменился.
//! Потребитель хранит номер, при котором раскладывал, и сравнивает: сбрасывать нечего.

use crate::dom::Element;
use crate::style::computed::{Display, Position};
use crate::style::values::value::Len;

/// Номера публикаций, на которых изменились входы раскладки коробки.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Epochs {
    /// Собственные входы: used-стиль раскладки, прочитанные раскладкой атрибуты,
    /// метки, строчное содержимое, список детей-коробок.
    pub(crate) layout: u32,
    /// У потомка изменились собственные входы (ставится всем предкам до корня).
    pub(crate) child: u32,
    /// Может измениться min/max-content вклад коробки в предка.
    pub(crate) intrinsic: u32,
    /// Изменилось что-либо, что читает сборка элементов коробки или её
    /// поддерева: входы раскладки или только краски (ставится всем предкам).
    pub(crate) paint: u32,
}

/// Почему коробка грязна на этой публикации.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Change {
    /// Новая, перемещённая или со сменой собственных входов: её вклад может
    /// измениться, даже если он задан извне (ширина 100px → 200px).
    Own,
    /// Изменились только дети или текст: вклад меняется, если он не внешний.
    Content,
    /// Изменились только значения краски: раскладка та же.
    Paint,
}

/// Вклад коробки в intrinsic-размеры предка не зависит от её содержимого:
/// inline-size и его min/max заданы длиной, отступы и рамка — тоже, или коробка
/// вынесена из потока. Ячейка и подпись таблицы, элемент flex/grid, таблица и
/// ортогональный поток не останавливают подъём: их вклад читает содержимое
/// (css-tables-3 §3.9, css-flexbox-1 §4.5, Servo
/// `outer_inline_content_sizes_depend_on_content`).
pub(super) fn external_contribution(e: &Element, parent: Option<&Element>) -> bool {
    let style = e.style.used();
    if matches!(
        style.reset_positioned.position,
        Some(Position::Absolute | Position::Fixed)
    ) {
        return true;
    }
    if let Some(parent) = parent {
        let p = parent.style.used();
        let item = matches!(
            p.reset_box_layout.display,
            Some(
                Display::Flex
                    | Display::InlineFlex
                    | Display::Grid
                    | Display::InlineGrid
                    | Display::GridLanes
            )
        );
        if item || p.inh.writing.writing_mode != style.inh.writing.writing_mode {
            return false;
        }
    }
    let layout = &style.reset_box_layout;
    // `display: inline` после каскада — `InlineBlock` с `inline_display`
    // (`layout::box_kind::inline_level_box`); у строчной коробки ширины нет.
    let floated = layout.float.is_some_and(|f| f != 0);
    let inline = !floated
        && (style.reset_context.inline_display == Some(true)
            || layout.display.is_none() && e.inline);
    let boxed = matches!(
        layout.display,
        None | Some(
            Display::Block
                | Display::ListItem
                | Display::InlineBlock
                | Display::Flex
                | Display::InlineFlex
                | Display::Grid
                | Display::InlineGrid
                | Display::GridLanes
        )
    );
    // Ячейка и таблица: их ширина — только нижняя граница (css-tables-3 §3.9);
    // табличные виды у HTML-элементов приходят по тегу, не по `display`.
    let table = matches!(
        e.tag.as_str(),
        "table" | "caption" | "td" | "th" | "tr" | "tbody" | "thead" | "tfoot"
    );
    if inline || !boxed || table {
        return false;
    }
    let fixed = |len: &Option<Len>| matches!(len, Some(Len::Px(_) | Len::Em(_) | Len::EmPx(..)));
    let bound = |len: &Option<Len>| len.is_none() || fixed(len) || matches!(len, Some(Len::Auto));
    let sides = |s: &crate::style::computed::Sides| {
        [&s.left, &s.right]
            .into_iter()
            .all(|l| l.is_none() || fixed(l))
    };
    fixed(&layout.width)
        && bound(&layout.min_width)
        && bound(&layout.max_width)
        && layout.calc_size.iter().all(Option::is_none)
        && sides(&layout.padding)
        && sides(&style.reset_border.border_width)
}

/// Атрибуты, которые раскладка не читает: класс и стиль доходят через
/// used-стиль, остальные — только для каскада, ссылок и доступности.
fn layout_reads(name: &str) -> bool {
    !matches!(name, "class" | "style" | "id" | "title" | "href")
        && !name.starts_with("aria-")
        && !name.starts_with("data-")
}

/// Одинаковые собственные входы раскладки у коробки двух публикаций (без детей).
/// В отличие от `dom::same_layout_box` (ключ повтора записей, который повторяет
/// и краску SVG-разметки), здесь важна только раскладка. `inherited`: у
/// родителей те же вычисленные стили (все предки прошли это сравнение), и
/// стиль ещё не вычисленной пересборки сравнивается по каскаду
/// (`dom::same_cascaded_layout_stages`, Blink `StyleDifference` до раскладки).
pub(super) fn same_layout_inputs(a: &Element, b: &Element, inherited: bool) -> bool {
    let read = |(name, _): &&(String, String)| layout_reads(name);
    a.node_id == b.node_id
        && a.tag == b.tag
        && a.inline == b.inline
        && a.list_item == b.list_item
        && match (&a.anim, &b.anim) {
            (None, None) => true,
            (Some(x), Some(y)) => std::rc::Rc::ptr_eq(x, y),
            _ => false,
        }
        && a.marks == b.marks
        && (a.attrs == b.attrs || a.attrs.iter().filter(read).eq(b.attrs.iter().filter(read)))
        && if inherited {
            crate::dom::same_cascaded_layout_stages(a, b)
        } else {
            crate::dom::same_layout_stages(a, b)
        }
}
