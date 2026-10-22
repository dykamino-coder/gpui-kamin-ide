//! Статическая позиция абсолютных коробок (CSS 2.1 §10.3.7, css-position-3
//! §static-position-rectangle): доля вдоль строки для строчного абсолюта по `text-align`,
//! доли самовыравнивания `justify-self`/`align-self` в прямоугольнике статической позиции
//! у блочного и строчного родителя, признак абсолюта без заданных краёв и расхождение
//! отсчёта края у замещаемого абсолюта из-за отбивки содержащего блока.

use crate::dom::Element;
use crate::layout::positioned::predicates::edge_set;
use crate::style::computed::{Align, Computed};
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

/// Доли выравнивания абсолюта в прямоугольнике статической позиции
/// (`containing_block::spot`, поле `self_align`). У блочного родителя прямоугольник по
/// строчной оси — края его содержимого, по блочной — нулевой, в самой
/// статической точке (css-position-3 §static-position-rectangle), и
/// `justify-self`/`align-self` выравнивают коробку в нём (css-align-3
/// §justify-abspos, §align-abspos; эталон `align-self-static-position-001`:
/// `end` — `top: -45px; left: 30px`, низ коробки в точке).
///
/// Только горизонтальное письмо родителя и блочный абсолют: у строчного
/// родителя и в вертикальном письме прямоугольник другой
/// (`align-self-static-position-002/003/004/007`) — `None`, прежний ход.
/// `None` и тогда, когда ни одна ось не задала своего выравнивания.
/// `normal`/`stretch`/`first baseline` у абсолюта ведут себя как `start`,
/// `last baseline` — как `end` (css-align-3 §9.3); `anchor-center` без
/// якоря — как `center` (css-anchor-position-1 §anchor-center), с якорем его
/// ведёт `anchor::place::place_one`, и сюда он не берётся.
pub(crate) fn static_self_align(e: &Element, inherited: &Computed) -> Option<(f32, f32)> {
    if e.style.inline_display == Some(true) {
        return None;
    }
    let anchorless = e.style.position_anchor.is_none()
        && e.style.position_area.is_none()
        && e.style.implicit_anchor.is_none();
    let frac = |a: Option<Align>, last: bool| match a {
        Some(Align::Center) => Some(0.5),
        Some(Align::AnchorCenter) if anchorless => Some(0.5),
        Some(Align::End) => Some(1.0),
        Some(Align::Baseline) if last => Some(1.0),
        Some(Align::Start) => Some(0.0),
        _ => None,
    };
    // `justify-self: auto` takes the parent's `justify-items` (css-align-3
    // §6.1; Blink `InlineStaticPositionEdge` with the block parent's style
    // as `justify_items_style`); `align-items` does not apply in block
    // layout, so `align-self: auto` stays `normal`
    // (`align-items-static-position-001`).
    let jx = match e.style.justify_self {
        None if !e.style.justify_self_normal => {
            frac(inherited.justify_items, inherited.justify_items_last)
        }
        own => frac(own, e.style.justify_self_last),
    };
    let ky = frac(e.style.align_self, e.style.align_self_last);
    if jx.is_none() && ky.is_none() {
        return None;
    }
    // Строчная ось: `start`/`end` — по письму родителя, `self-*` — по своему
    // (свой `direction` на этом шаге ещё не унаследован — берём родительский).
    let rtl = if e.style.justify_self_own_axis {
        e.style.rtl.or(inherited.rtl)
    } else {
        inherited.rtl
    } == Some(true);
    let kx = match jx {
        Some(k) if rtl => 1.0 - k,
        Some(k) => k,
        None if rtl => 1.0,
        None => 0.0,
    };
    let kb = ky.unwrap_or(0.0);
    // Vertical parent: the inline axis is vertical, the block axis horizontal
    // and its start is the right edge in `vertical-rl`. The fractions stay
    // PHYSICAL (x, y): the static-position rectangle is the parent's content
    // box in the inline axis and zero-thickness at the static point in the
    // block axis, whatever the writing mode (Blink
    // `BlockLayoutAlgorithm::HandleOutOfFlowPositioned` works in logical
    // offsets; `align-self-static-position-002`).
    if inherited.wm().vertical() {
        let kb = if inherited.wm().block_rl() {
            1.0 - kb
        } else {
            kb
        };
        return Some((kb, kx));
    }
    Some((kx, kb))
}

/// Self-alignment fractions `(x, y)` of a block-level absolute box whose
/// static position lies in a line of an INLINE formatting context: the
/// static-position rectangle is zero-thick along the line at the static point
/// and spans the line box across it (css-position-3 §staticpos-rect; Blink
/// `FragmentBuilder::AddOutOfFlowInlineChildCandidate`, which ignores
/// `justify-items`/`align-items`). Horizontal ltr only — other modes keep the
/// previous placement (`None`), as does `normal`/`auto` in both axes.
pub(crate) fn inline_static_self_align(e: &Element, inherited: &Computed) -> Option<(f32, f32)> {
    if inherited.wm().vertical()
        || inherited.wm().turned()
        || inherited.wm().rtl()
        || e.style.inline_display == Some(true)
    {
        return None;
    }
    let anchorless = e.style.position_anchor.is_none()
        && e.style.position_area.is_none()
        && e.style.implicit_anchor.is_none();
    let frac = |a: Option<Align>, last: bool| match a {
        Some(Align::Center) => Some(0.5),
        Some(Align::AnchorCenter) if anchorless => Some(0.5),
        Some(Align::End) => Some(1.0),
        Some(Align::Baseline) if last => Some(1.0),
        Some(Align::Start) => Some(0.0),
        _ => None,
    };
    let jx = frac(e.style.justify_self, e.style.justify_self_last);
    let ky = frac(e.style.align_self, e.style.align_self_last);
    if jx.is_none() && ky.is_none() {
        return None;
    }
    Some((jx.unwrap_or(0.0), ky.unwrap_or(0.0)))
}

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
