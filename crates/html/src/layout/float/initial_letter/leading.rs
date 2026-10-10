//! Ведущий флоат строки и строчный хост флоатов; ширина поля.

use crate::dom::{Element, Node};
use crate::layout::float::band_host::px_margin;
use crate::paint::effects::grouped::px_of2;
use crate::style::computed::{Computed, Display};
use crate::text::text_box::blank_text;

/// Флоат, ради которого строчная коробка и существует, — и ничего кроме него.
///
/// Содержащий блок флоата — ближайший БЛОЧНЫЙ предок (§10.1: «the containing
/// block is formed by the content edge of the nearest block container ancestor
/// box»), а не строчная коробка, внутри которой он записан. Правило 1 §9.5.1
/// держит его внешний край у края СОДЕРЖАЩЕГО БЛОКА, поэтому отбивка, рамка и
/// поле `<span>` флоат не двигают ни на точку. Сегодня двигают: `wrap_floats`
/// смотрит только на список братьев (проверка `floated` ниже не рекурсивная),
/// а разделение на строчное и блочное (`:3742`) исключает из прогона лишь
/// ПРЯМОГО плавающего ребёнка. Завёрнутый в `<span>` флоат уезжает в абзац и
/// встаёт от содержательного края строчной коробки — в `float-in-inline-001`
/// это ровно 30 + 30 + 40 = 100 точек вправо и вниз.
///
/// Возвращается сам флоат; строчная обёртка выбрасывается. Терять с ней
/// нечего: своего содержимого у неё нет, а рамку и отбивку строчной коробки
/// БЕЗ фона мы и так не рисуем (корень `INLINE-BOX-PAINT`) — поэтому гейт
/// требует отсутствия фона.
///
/// Проба (`target/scout-floatline-2026-09.md` §5.4): дерево ПОСЛЕ снятия
/// обёртки сходится с настоящими эталонами `ref-filled-green-200px-square` и
/// `ref-filled-green-100px-square` в 0.00 на всех трёх парах подкорня.
///
/// Гейт узкий нарочно: обёртка — НАСТОЯЩАЯ строчная (`display: inline` либо
/// строчный по тегу и без своего `display`), не позиционированная, без
/// `clear`, ничего не красящая и не образующая ГРУППУ, без стилей
/// `:hover`/`::first-letter`/`::first-line`, а внутри неё — только пустой
/// текст и РОВНО ОДИН элемент: флоат либо такая же обёртка
/// (`float-in-inline-002`: `<span><span><span style="float:left">`).
/// Leading float of an inline wrapper (see `wrap_floats`): the wrapper (or a
/// chain of such wrappers) must be a genuine, non-positioned inline that
/// forms no group (opacity, filter, transform, … act on the float through
/// it), and the float must be its first in-flow content — only blank text
/// before it. Returns the float and the wrapper without it.
pub(crate) fn split_leading_float(e: &Element) -> Option<(Element, Element)> {
    let genuine_inline =
        (e.inline && e.style.display.is_none()) || e.style.inline_display == Some(true);
    // Ruby boxes are not plain inline wrappers: their content is paired into
    // bases and annotations (css-ruby-1 §2.2) and laid out by the ruby path.
    if !genuine_inline
        || matches!(e.tag.as_str(), "br" | "ruby" | "rb" | "rt" | "rtc" | "rp")
        || e.style.float.is_some_and(|f| f != 0)
        || e.style.position.is_some()
        || e.hover.is_some()
        || e.style.opacity.is_some()
        || e.style.filter.is_some()
        || e.style.transform.is_some()
        || e.style.blend.is_some()
        || e.style.clip_polygon.is_some()
        || e.style.mask_image.is_some()
    {
        return None;
    }
    for (i, n) in e.children.iter().enumerate() {
        match n {
            Node::Text(t) if blank_text(t) => continue,
            Node::Text(_) => return None,
            Node::Element(c) => {
                if c.style.float.is_some_and(|f| f != 0)
                    && c.style.position.is_none()
                    && c.style.display != Some(Display::None)
                {
                    let mut rest = e.clone();
                    rest.children.remove(i);
                    // The float leaves its wrapper but keeps what it
                    // inherited through it (`run-in-contains-inline-007`:
                    // bold of the run-in).
                    let mut float = c.clone();
                    carry_inherited(&e.style, &mut float.style);
                    return Some((float, rest));
                }
                let (mut float, inner) = split_leading_float(c)?;
                carry_inherited(&e.style, &mut float.style);
                let mut rest = e.clone();
                rest.children[i] = Node::Element(inner);
                return Some((float, rest));
            }
        }
    }
    None
}

/// Inherited values a hoisted float takes from the inline wrapper it left
/// (only those the wrapper sets itself; `inline::inherit` would also resolve
/// font-relative units against the bare wrapper style).
pub(super) fn carry_inherited(wrapper: &Computed, own: &mut Computed) {
    own.color = own.color.or(wrapper.color);
    own.font_weight = own.font_weight.or(wrapper.font_weight);
    own.italic = own.italic.or(wrapper.italic);
    if own.font_family.is_none() {
        own.font_family = wrapper.font_family.clone();
    }
}

pub(crate) fn inline_float_host(e: &Element) -> Option<Element> {
    // `display: inline` после каскада — это `InlineBlock` с пометкой
    // `inline_display` (`computed.rs`), поэтому одного взгляда на `display`
    // мало; тег без своего `display` даёт строчность через `e.inline`.
    let genuine_inline =
        (e.inline && e.style.display.is_none()) || e.style.inline_display == Some(true);
    if !genuine_inline
        || e.style.float.is_some_and(|f| f != 0)
        || e.style.clear.is_some()
        || e.style.position.is_some()
        || e.hover.is_some()
        || e.first_letter.is_some()
        || e.first_line.is_some()
        // Обёртка КРАСИТ: фон, картинка и градиент ушли бы вместе с ней.
        || e.style.background.is_some()
        || e.style.bg_image.is_some()
        || e.style.gradient.is_some()
        // Обёртка образует ГРУППУ: прозрачность, фильтр, трансформ,
        // смешивание, обрезка и маска действуют на плавающего ребёнка ЧЕРЕЗ
        // неё. `css-color/inline-opacity-float-child` (зелёная, 0.00) держится
        // ровно на этом: `opacity: 0` на `<span>` гасит красный флоат внутри,
        // и снятая обёртка проявила бы красное.
        || e.style.opacity.is_some()
        || e.style.filter.is_some()
        || e.style.transform.is_some()
        || e.style.blend.is_some()
        || e.style.clip_polygon.is_some()
        || e.style.mask_image.is_some()
    {
        return None;
    }
    // Ровно один элемент и сколько угодно пустого текста. Непустой текст,
    // второй элемент, `<br>` — обёртка несёт СВОЁ содержимое, снимать её
    // нельзя: строчный прогон разъедется. Этим же условием из-под патча
    // выведены `below-float`, `float-nowrap-3/9` и `block-in-inline-margins-004`.
    let mut only: Option<&Element> = None;
    for n in &e.children {
        match n {
            Node::Text(t) if blank_text(t) => {}
            Node::Element(c) if only.is_none() => only = Some(c),
            _ => return None,
        }
    }
    let inner = only?;
    if inner.style.float.is_some_and(|f| f != 0) {
        return Some(inner.clone());
    }
    inline_float_host(inner)
}

/// Ширина margin-box по строчной оси в точках (`auto`-поле — ноль, как в
/// `px_margin`). Нижняя оценка для флоата без своей ширины: shrink-to-fit не
/// меньше нуля.
pub(crate) fn px_margin_w(c: &Computed) -> Option<f32> {
    let b = c.borders();
    Some(
        px_of2(&c.width)?
            + px_of2(&c.padding.left)?
            + px_of2(&c.padding.right)?
            + px_of2(&b.left)?
            + px_of2(&b.right)?
            + px_margin(&c.margin.left)?
            + px_margin(&c.margin.right)?,
    )
}
