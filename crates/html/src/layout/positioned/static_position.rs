//! Статическая позиция абсолютных коробок (CSS 2.1 §10.3.7, css-position-3
//! §static-position-rectangle): доля вдоль строки для строчного абсолюта по `text-align`,
//! доли самовыравнивания `justify-self`/`align-self` в прямоугольнике статической позиции
//! у блочного и строчного родителя, признак абсолюта без заданных краёв и расхождение
//! отсчёта края у замещаемого абсолюта из-за отбивки содержащего блока.

use crate::dom::Element;
use crate::layout::positioned::predicates::edge_set;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

/// Доля вдоль строки для статической точки СТРОЧНОГО абсолюта.
///
/// Гипотетическая коробка считается при `position: static` (CSS 2.1 §10.3.7),
/// а там `display: inline` остаётся строчным — значит коробка лежит в строке и
/// едет вместе с её выключкой. У БЛОЧНОГО абсолюта гипотетическая коробка —
/// блок, `text-align` его не двигает, поэтому гейт по `inline_display`:
/// метку «настоящий строчный» ставит каскад (`dom`), и блокификация
/// под `position: absolute` (§9.7, `dom::finish_inline_display`) её НЕ снимает.
/// `None` при выключке по началу строки — прежний ход без изменений.
pub(crate) fn static_line_align(e: &Element, inherited: &Computed) -> Option<f32> {
    if e.style.inline_display != Some(true) {
        return None;
    }
    let rtl = inherited.wm().rtl();
    match inherited
        .text_align
        .unwrap_or(crate::style::computed::TextAlign::Start)
        .physical(rtl)
    {
        crate::style::computed::TextAlign::Center => Some(0.5),
        crate::style::computed::TextAlign::Left => Some(0.0),
        crate::style::computed::TextAlign::Right => Some(1.0),
        // Выключка по ширине пустую строку не двигает — она как `start`.
        _ => None,
    }
}

mod alignment;
pub(crate) use alignment::{inline_static_self_align, static_self_align, static_self_align_style};

/// Расходятся ли у ЗАМЕЩАЕМОГО абсолюта два отсчёта края — от внутреннего края
/// рамки родителя и от его содержимого.
///
/// §10.1 п.4.2 дословно: «Otherwise, the containing block is formed by the
/// padding edge of the ancestor». Раскладка так и считает: абсолютный ребёнок в
/// taffy отсчитывается от РАМКИ коробки (`vendor/taffy/src/compute/block.rs`:
/// `absolute_position_inset = resolved_border + scrollbar_gutter`; во
/// `flexbox.rs` — «Insets are resolved against the container size minus
/// border»). Blink делает то же сжатием фрагмента на рамку
/// (`out_of_flow_layout_part.cc`: `padding_box_rect.Contract(… Borders() …)`).
///
/// Но замещаемый абсолют исключён из вывода коробки из строчного пробега и
/// остаётся `Piece::Atom` внутри анонимного ряда, а ряд стоит уже на
/// СОДЕРЖИМОМ родителя (`content_box_inset = border + padding`). Промах равен
/// ровно отбивке содержащего блока: снимки
/// `css3-background-origin-border-box{,--ref}.png` дают зелёный квадрат в
/// (10, 67) и (35, 92) — разница (+25, +25) device при DPR 1.25, то есть
/// (+20, +20) css = `padding: 20px`.
///
/// Признак узкий нарочно. Родитель обязан САМ быть содержащим блоком и иметь
/// ненулевую отбивку в точках: при нулевой отбивке ряд стоит там же, где
/// внутренний край рамки, и вывод куска из строки ничего бы не изменил, зато
/// разрезал бы строчный пробег. Отбивка в шрифтовых единицах сюда не входит —
/// на этом месте она ещё не сведена к точкам, и признак остаётся ложным.
pub(crate) fn cb_padding_shifts_replaced(e: &Element, inherited: &Computed) -> bool {
    if !matches!(e.tag.as_str(), "img" | "svg" | "canvas") {
        return false;
    }
    // Содержащий блок — именно РОДИТЕЛЬ. Если он не позиционирован, коробка
    // считает края от более далёкого предка или от области просмотра, и
    // отбивка родителя к делу не относится вовсе.
    if !crate::text::inline::establishes_cb(inherited) {
        return false;
    }
    let side = |l: Option<Len>| matches!(l, Some(Len::Px(v)) if v > 0.0);
    side(inherited.padding.top)
        || side(inherited.padding.right)
        || side(inherited.padding.bottom)
        || side(inherited.padding.left)
}

/// Абсолютный элемент, которому не задан ни один край.
///
/// Такой элемент по CSS остаётся на статической позиции — той, что была бы у
/// него в обычном потоке. Как только задан хотя бы один край, отсчёт идёт от
/// содержащего блока, и пустышка в строке уже не нужна.
pub(crate) fn at_static_position(c: &Computed) -> bool {
    // Доля считается от СОДЕРЖАЩЕГО БЛОКА, а пустышка нулевая: элемент с
    // `height: 100%` внутри неё схлопнулся бы в ноль. Такому оставляем прежнее
    // размещение — размер важнее точки отсчёта, его видно всегда.
    // Не снимать вето с доли по ИНЛАЙНОВОЙ оси, хотя по спеке прямоугольник
    // статической позиции нулевой лишь по одной оси (css-position-3, Blink
    // `absolute_utils.cc`): доля ширины у нас решается ОТ РАСПОРКИ, а не от
    // содержащего блока (`absolute-replaced-width-006/013/020`,
    // `margin-bottom-103/104`). Сперва нужен настоящий прямоугольник
    // статической позиции, потом снятие вето.
    let relative_size = matches!(c.width, Some(Len::Pct(_)))
        || matches!(c.height, Some(Len::Pct(_)))
        || matches!(c.min_width, Some(Len::Pct(_)))
        || matches!(c.min_height, Some(Len::Pct(_)))
        || matches!(c.max_width, Some(Len::Pct(_)))
        || matches!(c.max_height, Some(Len::Pct(_)));
    // Только `absolute`: у `fixed` содержащий блок — окно, и слой ему строит
    // сборщик дерева; пустышка в потоке ломала бы этот слой.
    c.position == Some(crate::style::computed::Position::Absolute)
        && !relative_size
        && !edge_set(c.inset.top)
        && !edge_set(c.inset.right)
        && !edge_set(c.inset.bottom)
        && !edge_set(c.inset.left)
}

/// Whether the box's hypothetical static box (`position: static`) is
/// inline-level: such a box stays a point in the line, a block-level one
/// starts at the line's start (CSS 2.1 §10.3.7, §10.6.4).
pub(crate) fn hypothetical_inline_level(e: &Element) -> bool {
    // A blockified `inline-block`/`inline-flex`/… keeps its inline-level
    // hypothetical box (`dom` marks it `abs_inline_level`).
    if e.style.abs_inline_level {
        return true;
    }
    match e.style.display {
        Some(crate::style::computed::Display::InlineBlock)
        | Some(crate::style::computed::Display::InlineFlex)
        | Some(crate::style::computed::Display::InlineGrid)
        | Some(crate::style::computed::Display::InlineTable) => true,
        Some(crate::style::computed::Display::GridLanes) => e.style.lanes_inline,
        // Блокифицированный `display: inline` (§9.7) для СТАТИЧЕСКОЙ
        // позиции остаётся строчным: гипотеза §10.3.7 считается без
        // блокификации (htb-rtl-*).
        Some(_) => e.style.inline_display == Some(true),
        None => e.inline,
    }
}
