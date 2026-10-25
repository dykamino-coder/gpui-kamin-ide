//! Ведущий флоат строки и строчный хост флоатов; ширина поля.

use crate::dom::whitespace::blank_text;
use crate::dom::{Element, Node};
use crate::layout::float::band_host::px_margin;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::px_missing_zero;

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
/// Возвращается флоат и обёртка без него (см. `wrap_floats`). Терять с ней
/// нечего: своего содержимого у неё нет, а рамку и отбивку строчной коробки
/// БЕЗ фона мы и так не рисуем (корень `INLINE-BOX-PAINT`) — поэтому гейт
/// требует отсутствия фона.
///
/// Проба: дерево ПОСЛЕ снятия
/// обёртки сходится с настоящими эталонами `ref-filled-green-200px-square` и
/// `ref-filled-green-100px-square` в 0.00 на всех трёх парах подкорня.
///
/// Гейт узкий нарочно: обёртка — НАСТОЯЩАЯ строчная (`display: inline` либо
/// строчный по тегу и без своего `display`), не позиционированная, без
/// `clear`, ничего не красящая и не образующая ГРУППУ, без стилей
/// `:hover`/`::first-letter`/`::first-line`, а внутри неё — только пустой
/// текст и РОВНО ОДИН элемент: флоат либо такая же обёртка
/// (`float-in-inline-002`: `<span><span><span style="float:left">`).
pub(crate) fn split_leading_float(e: &Element) -> Option<(Element, Element)> {
    let mut path = Vec::new();
    let mut current = e;
    loop {
        let genuine_inline = (current.inline && current.style.used().display.is_none())
            || current.style.used().inline_display == Some(true);
        // Ruby boxes are not plain inline wrappers: their content is paired into
        // bases and annotations (css-ruby-1 §2.2) and laid out by the ruby path.
        if !genuine_inline
            || matches!(
                current.tag.as_str(),
                "br" | "ruby" | "rb" | "rt" | "rtc" | "rp"
            )
            || current.style.used().float.is_some_and(|f| f != 0)
            || current.style.used().position.is_some()
            || current.hover.is_some()
            || current.style.used().opacity.is_some()
            || current.style.used().filter.is_some()
            || current.style.used().transform.is_some()
            || current.style.used().blend.is_some()
            || current.style.used().clip_polygon.is_some()
            || current.style.used().mask_image.is_some()
        {
            return None;
        }
        let (i, child) = current
            .children
            .iter()
            .enumerate()
            .find_map(|(i, n)| match n {
                Node::Text(t) if blank_text(t) => None,
                _ => Some((i, n)),
            })?;
        let Node::Element(child) = child else {
            return None;
        };
        path.push((current, i));
        if child.style.used().float.is_some_and(|f| f != 0)
            && child.style.used().position.is_none()
            && child.style.used().display != Some(Display::None)
        {
            let mut float = child.clone();
            let mut inner = None;
            for (wrapper, i) in path.into_iter().rev() {
                carry_inherited(wrapper.style.cascaded(), float.style.used_mut());
                let mut rest = wrapper.clone();
                if let Some(inner) = inner {
                    rest.children[i] = Node::Element(inner);
                } else {
                    rest.children.remove(i);
                }
                inner = Some(rest);
            }
            return Some((float, inner?));
        }
        current = child;
    }
}

/// Inherited values a hoisted float takes from the inline wrapper it left
/// (only those the wrapper sets itself; `style::cascade::inherit` would also resolve
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
    let mut e = e;
    loop {
        // `display: inline` после каскада — это `InlineBlock` с пометкой
        // `inline_display` (`dom::display_inheritance`), поэтому одного взгляда на `display`
        // мало; тег без своего `display` даёт строчность через `e.inline`.
        let genuine_inline = (e.inline && e.style.used().display.is_none())
            || e.style.used().inline_display == Some(true);
        if !genuine_inline
        || e.style.used().float.is_some_and(|f| f != 0)
        || e.style.used().clear.is_some()
        || e.style.used().position.is_some()
        || e.hover.is_some()
        || e.first_letter.is_some()
        || e.first_line.is_some()
        // Обёртка КРАСИТ: фон, картинка и градиент ушли бы вместе с ней.
        || e.style.used().background.is_some()
        || e.style.used().bg_image.is_some()
        || e.style.used().gradient.is_some()
        // Обёртка образует ГРУППУ: прозрачность, фильтр, трансформ,
        // смешивание, обрезка и маска действуют на плавающего ребёнка ЧЕРЕЗ
        // неё. `css-color/inline-opacity-float-child` (зелёная, 0.00) держится
        // ровно на этом: `opacity: 0` на `<span>` гасит красный флоат внутри,
        // и снятая обёртка проявила бы красное.
        || e.style.used().opacity.is_some()
        || e.style.used().filter.is_some()
        || e.style.used().transform.is_some()
        || e.style.used().blend.is_some()
        || e.style.used().clip_polygon.is_some()
        || e.style.used().mask_image.is_some()
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
        if inner.style.used().float.is_some_and(|f| f != 0) {
            return Some(inner.clone());
        }
        e = inner;
    }
}

/// Ширина margin-box по строчной оси в точках (`auto`-поле — ноль, как в
/// `px_margin`). Нижняя оценка для флоата без своей ширины: shrink-to-fit не
/// меньше нуля.
pub(crate) fn px_margin_w(c: &Computed) -> Option<f32> {
    let b = c.borders();
    Some(
        px_missing_zero(c.width)?
            + px_missing_zero(c.padding.left)?
            + px_missing_zero(c.padding.right)?
            + px_missing_zero(b.left)?
            + px_missing_zero(b.right)?
            + px_margin(&c.margin.left)?
            + px_margin(&c.margin.right)?,
    )
}
