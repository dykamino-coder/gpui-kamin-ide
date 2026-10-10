//! Хвост потока, накрытый флоатом: коробка хвоста и её место.

use crate::dom::{Element, Node};
use crate::layout::float::band_host::px_margin_box_em;
use crate::render::{inline_level_box, is_blank, own_context, replaced_inline};
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use crate::text::text_box::blank_text;

/// Пустой блок потока, который флоат обязан НАКРЫТЬ (§9.5).
///
/// §9.5 перечисляет ЗАКРЫТЫМ списком, чей border box флоат перекрывать не
/// смеет: таблица, блочный замещаемый элемент и коробка, образующая свой
/// контекст форматирования. Обычный блок потока в список не входит — его
/// коробка стоит там же, где стояла бы без флоата, а сужаются только её
/// СТРОКИ. Приложение E кладёт флоаты (шаг 5) поверх фонов блоков потока
/// (шаг 4), поэтому накрытая часть блока не видна.
///
/// Сегодня `wrap_floats` уводит такую пару на флекс-ряд, и блок встаёт СБОКУ
/// от флоата: в `clear-004` красный квадрат 100×100 выезжает на x = 100 и
/// виден целиком («красное видно» при эталоне «голый зелёный квадрат»).
///
/// Гейт узкий нарочно — берётся ровно тот случай, где итог считается
/// арифметикой, а не раскладкой: у соседа НЕТ строк (внутри только пустой
/// текст), его border box известен точками и ЦЕЛИКОМ ложится внутрь margin
/// box флоата. Тогда после правки на экране остаётся один флоат, и терять
/// нечего. Шире — конвейер F1-F9 (`bands.rs` плюс правила 3 и 7), там уже
/// откачены две лобовые правки (`render.rs:6301`, `:6434`).
///
/// Возвращает `((ширина, высота) margin box флоата, (ширина, высота) border
/// box соседа)`.
///
/// Проба (`target/scout-floatplace-2026-09.md` §5.1): деревья ПОСЛЕ правки
/// для `clear-004`, `block-formatting-contexts-016`, `floats-135` и
/// `floats-008` сведены с НАСТОЯЩИМИ эталонами корпуса и дали 0.00 все
/// четыре; обратный порядок (сосед поверх флоата) даёт «красное видно».
pub(super) fn covered_flow_tail(
    floater: &Element,
    tail: &Element,
    em: f32,
) -> Option<((f32, f32), (f32, f32))> {
    let zero = |l: &Option<Len>| matches!(l, None | Some(Len::Px(0.0)));
    let no_margins = |c: &Computed| {
        zero(&c.margin.top)
            && zero(&c.margin.right)
            && zero(&c.margin.bottom)
            && zero(&c.margin.left)
    };
    // Флоат: размер числом и никаких своих полей — в поля ляжет подъём
    // соседа. Позиционированный флоат и флоат с формой обтекания идут
    // прежним путём: у первого своя ось (`block-step-size-none-does-not-
    // establish-*`: `position: relative; z-index: -1`), у второго вырезы
    // считает `shape-flow`.
    if !no_margins(&floater.style)
        || floater.style.position.is_some()
        || floater.style.shape_outside.is_some()
    {
        return None;
    }
    let (fw, fh) = px_margin_box_em(&floater.style, em)?;
    // Сосед: обычный блок потока — не свой контекст, не замещаемый, не
    // элемент списка, без `clear`, без позиционирования, без своих полей и
    // без стилей `:hover`/`::first-letter`/`::first-line`.
    if !matches!(tail.style.display, None | Some(Display::Block))
        || own_context(tail)
        || replaced_inline(&tail.tag)
        || tail.list_item.is_some()
        || tail.style.clear.is_some()
        || tail.style.position.is_some()
        || !no_margins(&tail.style)
        || tail.hover.is_some()
        || tail.first_letter.is_some()
        || tail.first_line.is_some()
    {
        return None;
    }
    // Строк у соседа быть не должно: их §9.5 СУЖАЕТ, а не накрывает.
    if !tail.children.iter().all(is_blank) {
        return None;
    }
    let (tw, th) = px_margin_box_em(&tail.style, em)?;
    // Накрыт ЦЕЛИКОМ — только тогда итог правки известен заранее.
    (tw <= fw && th <= fh).then_some(((fw, fh), (tw, th)))
}

pub(super) fn covered_tail_box(
    em: f32,
    out: &[Node],
    floaters: &[Element],
    rest: &Vec<Node>,
) -> Option<((f32, f32), (f32, f32))> {
    {
        let mut only: Option<&Element> = None;
        let mut single = true;
        for n in rest {
            match n {
                Node::Text(t) if blank_text(t) => {}
                Node::Element(e) if only.is_none() => only = Some(e),
                _ => single = false,
            }
        }
        let run_before = out
            .iter()
            .rev()
            .find(|n| !is_blank(n))
            .is_some_and(|n| match n {
                Node::Element(e) => inline_level_box(e),
                Node::Text(_) => true,
            });
        match (single && !run_before, only, floaters.first()) {
            (true, Some(tail), Some(f)) => covered_flow_tail(f, tail, em),
            _ => None,
        }
    }
}
