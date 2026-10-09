//! Режимы письма: ортогональные потоки, вертикальная раскладка.
// owner: A

use crate::dom::Element;
use crate::render::in_flow;
use crate::style::computed::{Computed, Display};
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div};

pub(crate) mod orthogonal_inline;
pub(crate) mod native_vertical;
pub(super) mod orthogonal_fixed_child;
pub(crate) mod orthogonal_children;
mod orthogonal_horizontal;
mod orthogonal_absolute;
mod vertical_intrinsic;
pub(crate) mod vertical_hug;
pub(crate) mod native_intrinsic;
mod physical_atomic;
pub(crate) mod rotated_atom;
mod physical_atomic_frame;
pub(crate) mod orthogonal_measure;

/// Блок вертикального письма занимает по горизонтали столько, сколько просит
/// содержимое, а не всю строку родителя.
///
/// Горизонтальная ось для него — ось ПОТОКА, а не строки: в Chrome контейнер
/// из трёх полос шириной 22 с отступами 16 занял 130 точек, а не всю ширину
/// окна. Блочная раскладка растягивает детей по ширине и слушать `align-self`
/// не обязана, поэтому обёртка-ряд: сам ряд занимает строку, а блок внутри
/// него жмётся к содержимому. Без этого колонки `vertical-rl` уезжали к
/// правому краю окна.
///
/// Приём этот — целиком про БЛОЧНЫЙ контейнер. css-writing-modes-4 §7.3 делит
/// раскладку ортогональной коробки надвое и про вторую половину говорит: «In
/// the positioning phase—calculating the positioning offsets, margins, borders,
/// and padding—…calculations are performed according to the writing mode of the
/// *containing block* of the box establishing the orthogonal flow»; §7.4
/// повторяет то же про «any properties related to positioning the box within
/// its containing block». А §7.3.3 включает подбор по содержимому только «when
/// the available inline space is infinite» — у элемента сетки и гибкого
/// элемента место определённое, и подбирать нечего.
pub(crate) fn vertical_hug(el: AnyElement, e: &Element, inherited: &Computed) -> AnyElement {
    let starts_here = e.style.vertical == Some(true) && inherited.vertical != Some(true);
    if !starts_here || e.style.width.is_some() {
        return el;
    }
    // Письмо, заданное на `body` (или `html`), — ГЛАВНОЕ письмо страницы: оно
    // управляет окном целиком, и содержимое `vertical-rl` начинается от
    // правого края окна, а не от края сжатой коробки.
    if matches!(e.tag.as_str(), "body" | "html") {
        return el;
    }
    // An absolutely positioned box is out of flow and is placed against the
    // padding box of its containing block (CSS 2.1 §10.1, css-position-3
    // §4). The in-flow hug row sits at the parent's CONTENT edge and became
    // the box's layout parent, so `top: 0; left: 0` landed inside the padding
    // (`available-size-001`: the vertical-rl `#red` 1ch below the green 0).
    if matches!(
        e.style.position,
        Some(crate::style::computed::Position::Absolute) | Some(crate::style::computed::Position::Fixed)
    ) {
        return el;
    }
    // Элемент СЕТКИ и ГИБКИЙ элемент размер по блочной оси не подбирают: его
    // задаёт выравнивание, и решается оно письмом КОНТЕЙНЕРА (§7.3,
    // «positioning phase … according to the writing mode of the containing
    // block»). Blink выбирает пару свойств прямо по письму сетки
    // (`grid/grid_item.cc`: `is_for_columns == IsParallelWritingMode(
    // root_grid_writing_direction…, parent_grid_style…) ? ResolvedJustifySelf
    // : ResolvedAlignSelf`), а `normal` у незамещаемой коробки даёт
    // `kStretchImplicit` -> `Length::Stretch()` (`length_utils.cc`), то есть
    // размер дорожки: ортогональность тут не при чём вовсе.
    //
    // Обёртка-ряд эту развязку ломала дважды. В сетке элементом дорожки
    // становилась ОНА, а блок внутри обнимал содержимое: снимок
    // `orthogonal-positioned-grid-items-009--ref` — magenta (29..55, 103..289)
    // px снимка = 21.6x150 CSS вместо дорожки 200x150 (тот же прямоугольник на
    // ТЕСТЕ, где обёртки нет, отрисован точно: 29..278). В гибком контейнере
    // обёртка становилась гибким элементом, а `flex:` оставалось на коробке
    // ВНУТРИ неё, и ряд не рос: снимок `flexbox-mbp-horiz-002v` — жёлтый
    // (13..29) = 12.8 CSS против 193 у эталона.
    //
    // Строчная ось у такой коробки уже разведена по выравниванию в
    // `orthogonal_vertical_children` (`hug_inline` при НЕрастягивающем
    // `align-self`); это — недостающая парная развязка блочной оси, и после
    // снятия обёртки её ведёт сама раскладка по `justify-self`/`justify-items`
    // (`apply.rs`; `align_keyword` отдаёт `None` на `normal`, и решает taffy).
    //
    // Абсолютный ребёнок сетки элементом сетки НЕ является (css-grid-1 §9) —
    // его размер задают вставки, обёртка ему и не мешала; `in_flow` его здесь
    // и отсекает. Лунки (`GridLanes`) не включены сознательно: у них свой
    // второй проход дорожек и 23 зелёные пары, это отдельный замер.
    if matches!(
        inherited.display,
        Some(Display::Grid)
            | Some(Display::InlineGrid)
            | Some(Display::Flex)
            | Some(Display::InlineFlex)
    ) && in_flow(&e.style)
    {
        return el;
    }
    // ЗАМЕРЕНО (не гипотеза): по этим тестам размер по оси строки — не корень
    // зла. Пробовал и растяжение на высоту родителя, и свой элемент-измеритель
    // (`fit-content` с зажимом по доступному) — на двенадцати тестах
    // ортогональных потоков сдвиг в пределах полупроцента. Настоящая поломка
    // видна замером против Chrome на `sizing-orthogonal-percentage-margin-001`:
    // элемент рисуется ПОЛОСОЙ 25×417 у левого края страницы, а должен быть
    // 100×100 внутри контейнера с полями 50. То есть вертикальный блок уходит
    // из коробки родителя — вот что чинить дальше.
    // The row is an adapter for normal block flow, not a CSS flex item.
    // Its own shrink default must not compress the child's used inline size:
    // sizing-orthog-vlr-in-htb-007 measured 306px instead of 394px content.
    div().flex().flex_row().flex_shrink_0().child(el).into_any_element()
}

/// Знак стоит прямо в вертикальном письме с `text-orientation: mixed`
/// (UTR#50, vo=U, упрощённо): иероглифика, кана, CJK-знаки препинания и
/// полноширинные формы. Остальное — лежит боком.
pub(crate) fn upright_in_mixed(c: char) -> bool {
    matches!(c as u32,
        0x3000..=0x303F   // CJK-знаки и пунктуация (「」、。 …)
        | 0x3040..=0x30FF // хирагана и катакана
        | 0x31F0..=0x31FF // фонетические расширения каны
        | 0x3200..=0x33FF // обведённые и совместимые CJK
        | 0x3400..=0x4DBF // иероглифика, расширение A
        | 0x4E00..=0x9FFF // иероглифика единая
        | 0xAC00..=0xD7AF // хангыль
        | 0xF900..=0xFAFF // совместимая иероглифика
        | 0xFE30..=0xFE4F // вертикальные формы совместимости
        | 0xFF01..=0xFF60 // полноширинные формы
        | 0x20000..=0x2FFFD // иероглифика, плоскость 2
    )
}
