//! Стадия коробки, самовыравнивание: `normal` у элемента flex = `stretch`, оси `self-*` по письму
//! самого элемента, концы письма под `wrap-reverse` родителя (css-align-3 §6.1–6.2).

use super::*;
use crate::style::computed::{Align, BoxStyle, Display as D};

fn flip(a: Option<Align>) -> Option<Align> {
    match a {
        Some(Align::Start) => Some(Align::End),
        Some(Align::End) => Some(Align::Start),
        other => other,
    }
}

pub(super) fn self_alignment(parent: &Computed, own: &Computed, c: &mut BoxStyle) {
    let flex_parent = matches!(parent.display, Some(D::Flex) | Some(D::InlineFlex));
    // Не снимать `align-self` у коробки, чей родитель не гибкий контейнер и
    // не сетка (css-align-3 §6.2 «does not apply to block-level boxes»): наш
    // блок собран колонкой flex, и на `align_self` держатся приёмы сборки —
    // соотношение сторон блока (`blocks()` ставит `Align::Start`), обтекание,
    // сжатие стола (`float-applies-to-*`, `floats-002/025/147`,
    // `clear-float-001/003`, `block-aspect-ratio-*`,
    // `shape-outside-*-border-radius-*`, `absolute-replaced-width-020/034`).
    // Годится только с признаком «значение авторское», чтобы приёмы сборки
    // гейт не задевал (`self-align-start-end-flex-001` — цель правки).
    // `normal` у элемента ГИБКОГО контейнера = `stretch` (§6.2), а не
    // «пусто»: пустое значение брало `align-items` родителя
    // (`self-align-normal-flex`).
    if own.align_self_normal && flex_parent {
        set!(c.align_self, Some(Align::Stretch));
    }
    // `self-start`/`self-end` меряются по письму САМОГО элемента (css-align-3
    // §6.2). Значение уже физическое (начало = левый край при ltr), поэтому
    // зеркалим ровно тогда, когда строчная ось элемента смотрит в другую
    // сторону, чем у родителя: `flexbox-align-self-vert-002` даёт элементам
    // `direction: rtl` внутри ltr-колонки и ждёт `self-start` СПРАВА.
    // Вертикальное письмо здесь НЕ зеркалим намеренно: там ось строки уже
    // переставлена поворотом. Письмо элемента — ДЕЙСТВУЮЩЕЕ (своё или
    // унаследованное, вычисленное `direction`): незаданное `direction` у
    // элемента в rtl-колонке — тоже rtl, и зеркалить нечего (поперечную ось
    // rtl-колонки разворачивает сама раскладка, `paint/apply/`:
    // `flex_cross_reverse` / `flip`).
    if c.parent_grid == 0 && own.align_self_own_axis && c.rtl.unwrap_or(false) != parent.wm().rtl()
    {
        set!(c.align_self, flip(c.align_self));
    }
    // `start`/`end` (и `self-*`) меряются по ПИСЬМУ, а раскладка знает только
    // гибкие концы (`apply::to_items` → `FlexStart`/`FlexEnd`), которые
    // АВТОРСКИЙ `wrap-reverse` родителя переворачивает (css-align-3 §6.1,
    // css-flexbox-1 §5.2). Концы письма меняются местами ровно тогда: taffy
    // развернёт их обратно. Переворот поперёк письма (`paint/apply/`: `flip`
    // через тот же `WrapReverse`) здесь не участвует — он виден раскладке и
    // для `flex-*`, и для `start` одинаково (`self-align-start-end-flex-001`).
    if !own.align_self_flex_kw && parent.flex_wrap_reverse == Some(true) && flex_parent {
        set!(c.align_self, flip(c.align_self));
    }
}
