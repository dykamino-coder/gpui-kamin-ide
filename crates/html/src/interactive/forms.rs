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
use choice::select;
use choice::toggle;

use crate::dom::Element;
use crate::style::apply::apply;
use crate::style::computed::Computed;
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, div, px, rgb};

/// Цвета «по умолчанию» для элементов формы: документ обычно их не задаёт, а
/// голая рамка без фона на тёмной теме не читается.
const BORDER: u32 = 0x4a4a5a;
const FIELD_BG: u32 = 0x22232e;
const ACCENT: u32 = 0x3b5bdb;
const MUTED: u32 = 0x8a90a4;

/// Отрисовать элемент формы. `None` — тег не относится к формам.
pub fn element(
    e: &Element,
    style: &Computed,
    opts: &crate::render::RenderOpts,
) -> Option<AnyElement> {
    match e.tag.as_str() {
        "input" => Some(input(e, style)),
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

fn input(e: &Element, style: &Computed) -> AnyElement {
    match kind(e).as_str() {
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
            .unwrap_or_else(|| rgb(0xd4d4d4).into()))
    });
    field_box(style)
        .child(
            div()
                .when_muted(muted)
                .child(SharedString::from(text))
                .into_any_element(),
        )
        .children(caret)
        .into_any_element()
}

fn textarea(e: &Element, style: &Computed, opts: &crate::render::RenderOpts) -> AnyElement {
    let mut text = String::new();
    crate::render::gather_text_public(&e.children, &mut text);
    let muted = text.trim().is_empty();
    // Высота — по числу строк, как `rows` в HTML: без этого поле схлопывается
    // до одной строки и перестаёт быть похожим на себя.
    let rows: f32 = e.attr("rows").and_then(|r| r.parse().ok()).unwrap_or(3.0);
    // Содержимое поля — обычный текст, и считать его обязана та же строчная
    // раскладка, что и весь остальной текст: у поля работают и сохранённые
    // пробелы, и выключка, и висящий хвост. Пока внутри стоял простой блок,
    // всё это проходило мимо (`trailing-space-and-text-alignment`).
    let body = if muted {
        let hint = e.attr("placeholder").unwrap_or("").to_string();
        div()
            .when_muted(true)
            .child(SharedString::from(hint))
            .into_any_element()
    } else {
        crate::render::paragraph_public(&e.children, style, opts)
    };
    let _ = text;
    field_box(style)
        .min_h(px(rows * 18.0 + 12.0))
        .items_start()
        .child(div().w_full().child(body))
        .into_any_element()
}

/// `<input type="button">` — значение лежит в атрибуте, а не в детях.
fn button_like(e: &Element, style: &Computed) -> AnyElement {
    apply(div(), style)
        .px(px(10.))
        .py(px(4.))
        .rounded(px(4.))
        .bg(rgb(FIELD_BG))
        .border_1()
        .border_color(rgb(BORDER))
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
    let mut d = apply(div(), style).flex().items_center().min_h(px(24.));
    // Служебная заливка поля — только когда автор о фоне НЕ говорил:
    // `background: linear-gradient(...)` сбрасывает цвет в прозрачный, и
    // заливка поверх него закрашивала бы страницу под полем.
    if !style.bg_explicit && style.background.is_none() && style.gradient.is_none() {
        d = d.bg(rgb(FIELD_BG));
    }
    // «Автор ничего не сказал» — это когда не заданы НИ толщина, НИ рисунок.
    // По `borders()` отличить нельзя: он гасит толщину без рисунка, и
    // `border: none` выглядел бы как отсутствие правила — умолчание UA
    // возвращало рамку обратно.
    if style.border_width.top.is_none() && style.border_visible[0].is_none() {
        d = d.border_1();
    }
    if style.border_color.is_none() {
        d = d.border_color(rgb(BORDER));
    }
    if style.radius.tl.is_none() {
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
    fn when_muted(self, muted: bool) -> Self;
}

impl Muted for gpui::Div {
    fn when_muted(self, muted: bool) -> Self {
        if muted {
            self.text_color(rgb(MUTED))
        } else {
            self
        }
    }
}

/// Авторский цвет акцента формы, общий для выбора и индикаторов.
pub(super) fn control_accent(style: &Computed) -> Option<crate::style::values::value::Color> {
    style.accent_color
}
