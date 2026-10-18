//! Элементы форм: поля, флажки, переключатели, списки выбора.
//!
//! Важная оговорка вперёд: это **отрисовка, а не поведение**. Поле показывает
//! своё значение, флажок — своё состояние, но ввод и переключение приходят от
//! приложения, а не от документа. Причина проста: у нас нет и не может быть
//! исполнения скриптов, а без него форма всё равно никуда не отправится.
//!
//! Зачем тогда рисовать. Формы встречаются в разметке постоянно — настройки,
//! фильтры, макеты интерфейса от модели. Пустое место вместо поля выглядит
//! поломкой; нарисованное поле честно показывает задуманный вид.

mod indicators;
use indicators::color_swatch;
use indicators::progress;
use indicators::range;

mod choice;
mod metrics;
mod textarea;
use textarea::textarea;
mod options;
use choice::select;
use choice::toggle;

use crate::dom::Element;
use crate::style::apply::apply;
use crate::style::computed::Computed;
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, div, px};

use crate::style::values::value::{SystemColor, system_color};

/// Native controls share the replaced-content natural measurement mechanism.
fn control(paint: gpui::Div, natural: [f32; 2]) -> AnyElement {
    crate::layout::replaced::natural_control(paint, gpui::size(px(natural[0]), px(natural[1])))
}

fn native_color(style: &Computed, role: SystemColor) -> gpui::Hsla {
    system_color(role, style.dark_color_scheme).to_hsla()
}

/// Отрисовать элемент формы. `None` — тег не относится к формам.
pub fn element(
    e: &Element,
    style: &Computed,
    opts: &crate::render::RenderOpts,
) -> Option<AnyElement> {
    match e.tag.as_str() {
        "input" => Some(input(e, style, opts)),
        "textarea" => Some(textarea(e, style, opts)),
        "select" => Some(select(e, style)),
        "progress" => Some(progress(e, style)),
        "meter" => Some(progress(e, style)),
        _ => None,
    }
}

fn kind(e: &Element) -> String {
    e.attr("type").unwrap_or("text").to_ascii_lowercase()
}

fn input(e: &Element, style: &Computed, opts: &crate::render::RenderOpts) -> AnyElement {
    let kind = kind(e);
    // `appearance: none` (css-ui-4 §7.2): a checkbox, radio, range or colour
    // control loses its native widget and is a primitive box painted from its
    // CSS alone, with generated children in ordinary flow, no UA size (Blink
    // `LayoutTheme::AdjustStyle` skips the theme part for `none`).
    if style.appearance_none() && matches!(kind.as_str(), "checkbox" | "radio" | "range" | "color")
    {
        return apply(div(), style)
            .children(crate::render::blocks(&e.children, style, opts))
            .into_any_element();
    }
    match kind.as_str() {
        "checkbox" => toggle(e, style, false),
        "radio" => toggle(e, style, true),
        "button" | "submit" | "reset" => button_like(e, style),
        "range" => range(e, style),
        "color" => color_swatch(e, style),
        "hidden" => div().into_any_element(),
        _ => text_field(e, style),
    }
}

/// Текстовое поле: значение, иначе подсказка приглушённым цветом.
fn text_field(e: &Element, style: &Computed) -> AnyElement {
    let used = metrics::text_style(style);
    let style = &used;
    let (text, muted) = match (e.attr("value"), e.attr("placeholder")) {
        (Some(v), _) if !v.is_empty() => (v.to_string(), false),
        (_, Some(p)) => (p.to_string(), true),
        _ => (String::new(), true),
    };
    // `caret-color` видна только в поле с фокусом — рисуем её ровно там,
    // где фокус объявлен разметкой, иначе каретка стояла бы в каждом поле.
    let caret = e.attr("autofocus").is_some().then(|| {
        div().w(px(1.)).h(px(14.)).ml(px(1.)).bg(style
            .caret_color
            .or(style.color)
            .map(|c| c.to_hsla())
            .unwrap_or_else(|| native_color(style, SystemColor::FieldText)))
    });
    field_box(style)
        .child(
            div()
                .when_muted(muted, style)
                .child(SharedString::from(text))
                .into_any_element(),
        )
        .children(caret)
        .into_any_element()
}

/// `<input type="button">` — значение лежит в атрибуте, а не в детях.
fn button_like(e: &Element, style: &Computed) -> AnyElement {
    if style.appearance == Some(crate::style::computed::Appearance::Base) {
        return apply(div(), style)
            .child(SharedString::from(e.attr("value").unwrap_or("").to_owned()))
            .into_any_element();
    }
    apply(div(), style)
        .px(px(10.))
        .py(px(4.))
        .rounded(px(4.))
        .bg(native_color(style, SystemColor::ButtonFace))
        .border_1()
        .border_color(native_color(style, SystemColor::ButtonBorder))
        .child(SharedString::from(
            e.attr("value").unwrap_or("Кнопка").to_string(),
        ))
        .into_any_element()
}

/// Общая рамка поля ввода.
/// Общая рамка поля ввода.
///
/// Умолчания ставятся ТОЛЬКО там, где документ ничего не сказал: раньше они
/// шли безусловно и затирали фон, рамку, скругление и отступы из CSS.
fn field_box(style: &Computed) -> gpui::Div {
    if style.appearance == Some(crate::style::computed::Appearance::Base) {
        return apply(div(), style);
    }
    let mut d = apply(div(), style).flex().items_center().min_h(px(24.));
    // Служебная заливка поля — только когда автор о фоне НЕ говорил:
    // `background: linear-gradient(...)` сбрасывает цвет в прозрачный, и
    // заливка поверх него закрашивала бы страницу под полем.
    if !style.bg_explicit && style.background.is_none() && style.gradient.is_none() {
        d = d.bg(native_color(style, SystemColor::Field));
    }
    // «Автор ничего не сказал» — это когда не заданы НИ толщина, НИ рисунок.
    // По `borders()` отличить нельзя: он гасит толщину без рисунка, и
    // `border: none` выглядел бы как отсутствие правила — умолчание UA
    // возвращало рамку обратно.
    if style.border_width.top.is_none() && style.border_visible[0].is_none() {
        d = d.border_1();
    }
    if style.border_color.is_none() {
        d = d.border_color(native_color(style, SystemColor::ButtonBorder));
    }
    // The rounded native look is part of `appearance: auto`; an author
    // border or background replaces it with a plain CSS box (CSS UI 4
    // section 7.2, HTML rendering section 15.5), so its corners stay square.
    let author_box = style.border_width.top.is_some()
        || style.border_visible[0].is_some()
        || style.bg_explicit
        || style.background.is_some();
    if style.radius.tl.is_none() && !author_box {
        d = d.rounded(px(4.));
    }
    if style.padding.left.is_none() {
        d = d.px(px(8.));
    }
    if style.padding.top.is_none() {
        d = d.py(px(4.));
    }
    d
}

/// Приглушить текст подсказки — иначе она неотличима от введённого значения.
trait Muted {
    fn when_muted(self, muted: bool, style: &Computed) -> Self;
}

impl Muted for gpui::Div {
    fn when_muted(self, muted: bool, style: &Computed) -> Self {
        if muted {
            self.text_color(native_color(style, SystemColor::GrayText))
        } else {
            self
        }
    }
}

/// Авторский цвет акцента формы, общий для выбора и индикаторов.
pub(super) fn control_accent(style: &Computed) -> Option<crate::style::values::value::Color> {
    style.accent_color
}
