//! Разбор выравнивания (css-align-3): ключевые слова align-*/justify-*, safe/unsafe.

use super::*;

pub(in crate::style::computed) fn parse_align(v: &str) -> Option<Align> {
    align_keyword(v).unwrap_or(None)
}

/// Несёт ли значение выравнивания модификатор `safe` (css-align §5.3).
pub(in crate::style::computed) fn is_safe(v: &str) -> bool {
    v.split_whitespace().next() == Some("safe")
}

/// Значение выравнивания по css-align-3.
///
/// Исходов три, а не два. `Ok(Some)` — выравнивание задано; `Ok(None)` —
/// значение верное, но своего выравнивания не несёт (`normal`, `auto`), и поле
/// надо ОЧИСТИТЬ, чтобы решала раскладка; `Err(())` — объявление негодное,
/// и прежнее значение остаётся. Раньше эти три случая делились на два
/// по-разному у разных свойств: у `align-items` негодное значение сохраняло
/// прежнее, у `justify-items` — стирало его.
pub(in crate::style::computed) fn align_keyword(v: &str) -> Result<Option<Align>, ()> {
    let mut it = v.split_whitespace();
    let mut word = it.next().ok_or(())?;
    // `safe`/`unsafe` — что делать при переполнении области; сама позиция от
    // этого не меняется.
    if matches!(word, "safe" | "unsafe") {
        word = it.next().ok_or(())?;
    }
    Ok(match word {
        "center" => Some(Align::Center),
        // §anchor-center (только `align-self`/`justify-self`; у `*-items`
        // значение отброшено спекой — `apply` его там игнорирует).
        "anchor-center" => Some(Align::AnchorCenter),
        // `self-start`/`self-end` считаются по письму САМОГО элемента,
        // `start`/`end` — по письму контейнера (css-align-3 §6.2). Разница
        // видна, как только элемент несёт своё `direction`/`writing-mode`:
        // `flexbox-align-self-vert-002` ждёт `self-start` СПРАВА у элемента
        // с `direction: rtl`. Само значение остаётся физическим, а «мерить
        // по себе» помнится отдельным флагом `align_self_own_axis` — его
        // зеркалит `inline::inherit`, где известны письмо элемента И письмо
        // родителя.
        "start" | "flex-start" | "left" => Some(Align::Start),
        "end" | "flex-end" | "right" => Some(Align::End),
        "self-start" => Some(Align::Start),
        "self-end" => Some(Align::End),
        "stretch" => Some(Align::Stretch),
        // `first baseline` — обычное выравнивание по базовой линии. `last`
        // честно раскладке неизвестен, но для ОДНОСТРОЧНЫХ участников первая
        // и последняя базовые совпадают — суррогат первой ближе очистки
        // (flex-order-last-baseline; прежний None ронял участника в stretch).
        "baseline" | "first" | "last" => Some(Align::Baseline),
        // `normal` у растяжимого элемента даёт растяжение, а у замещаемого —
        // прижим к началу. Выбирает это сама раскладка, когда поле пусто.
        "normal" | "auto" => None,
        _ => return Err(()),
    })
}

pub(in crate::style::computed) fn parse_justify(v: &str) -> Option<Justify> {
    // `safe`/`unsafe` говорят, что делать при переполнении области; позиция
    // от этого не меняется, поэтому приставка снимается.
    let v = v
        .strip_prefix("safe ")
        .or_else(|| v.strip_prefix("unsafe "))
        .unwrap_or(v)
        .trim();
    match v {
        "center" => Some(Justify::Center),
        "flex-start" => Some(Justify::Start),
        "flex-end" => Some(Justify::End),
        // `left`/`right` физические; при письме слева направо они совпадают
        // с началом и концом строки.
        //
        // ЗАМЕРЕНО И ОТКАЧЕНО: развести их в отдельные значения, чтобы при
        // rtl они НЕ переставлялись вместе с `start`/`end` (css-align-3 §4).
        // Правка верна по спеке, но полный свод обоих: 0 и 0 —
        // `flexbox_justifycontent-right-002` (8.53) держит не это.
        "start" => Some(Justify::WmStart),
        "end" => Some(Justify::WmEnd),
        "left" => Some(Justify::Left),
        "right" => Some(Justify::Right),
        "space-between" => Some(Justify::Between),
        "space-around" => Some(Justify::Around),
        "space-evenly" => Some(Justify::Evenly),
        "stretch" => Some(Justify::Stretch),
        _ => None,
    }
}
