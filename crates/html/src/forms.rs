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

use crate::apply::apply;
use crate::computed::Computed;
use crate::dom::Element;
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, div, px, rgb};

/// Цвета «по умолчанию» для элементов формы: документ обычно их не задаёт, а
/// голая рамка без фона на тёмной теме не читается.
const BORDER: u32 = 0x4a4a5a;
const FIELD_BG: u32 = 0x22232e;
const ACCENT: u32 = 0x3b5bdb;
const MUTED: u32 = 0x8a90a4;

/// Отрисовать элемент формы. `None` — тег не относится к формам.
pub fn element(e: &Element, style: &Computed, opts: &crate::RenderOpts) -> Option<AnyElement> {
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

fn textarea(e: &Element, style: &Computed, opts: &crate::RenderOpts) -> AnyElement {
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

/// Список выбора: показываем выбранный вариант либо первый.
fn select(e: &Element, style: &Computed) -> AnyElement {
    let mut chosen: Option<String> = None;
    let mut first: Option<String> = None;
    for child in &e.children {
        let crate::dom::Node::Element(opt) = child else {
            continue;
        };
        if opt.tag != "option" {
            continue;
        }
        let mut label = String::new();
        crate::render::gather_text_public(&opt.children, &mut label);
        let label = label.trim().to_string();
        if first.is_none() {
            first = Some(label.clone());
        }
        if opt.attr("selected").is_some() {
            chosen = Some(label);
        }
    }
    let text = chosen.or(first).unwrap_or_default();
    field_box(style)
        .justify_between()
        .child(SharedString::from(text))
        // Стрелка рисуется символом: своей иконки у документа нет, а без неё
        // список неотличим от обычного поля.
        .child(div().text_color(rgb(MUTED)).child(SharedString::from("⌄")))
        .into_any_element()
}

/// Флажок и переключатель отличаются только скруглением и отметкой.
fn toggle(e: &Element, style: &Computed, round: bool) -> AnyElement {
    let on = e.attr("checked").is_some();
    // `accent-color` красит именно отметку флажка и переключателя — это
    // единственное, на что оно влияет.
    let accent: gpui::Hsla = style
        .accent_color
        .map(|c| c.to_hsla())
        .unwrap_or_else(|| rgb(ACCENT).into());
    let mut mark = apply(div(), style)
        .w(px(15.))
        .h(px(15.))
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(if on { accent } else { rgb(BORDER).into() })
        .bg(if on { accent } else { rgb(FIELD_BG).into() });
    mark = if round {
        mark.rounded_full()
    } else {
        mark.rounded(px(3.))
    };
    if on {
        mark = mark.child(
            div()
                .text_size(px(10.))
                .text_color(rgb(0xffffff))
                .child(SharedString::from(if round { "•" } else { "✓" })),
        );
    }
    mark.into_any_element()
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

/// Ползунок: дорожка и заполненная часть по значению.
fn range(e: &Element, style: &Computed) -> AnyElement {
    let num = |name: &str, default: f32| -> f32 {
        e.attr(name).and_then(|v| v.parse().ok()).unwrap_or(default)
    };
    let (min, max, val) = (num("min", 0.), num("max", 100.), num("value", 50.));
    let accent: gpui::Hsla = style
        .accent_color
        .map(|c| c.to_hsla())
        .unwrap_or_else(|| rgb(ACCENT).into());
    let frac = if max > min {
        ((val - min) / (max - min)).clamp(0., 1.)
    } else {
        0.
    };
    apply(div(), style)
        .h(px(16.))
        .min_w(px(60.))
        .flex()
        .items_center()
        .child(
            div()
                .relative()
                .w_full()
                .h(px(4.))
                .rounded(px(2.))
                .bg(rgb(BORDER))
                .child(
                    div()
                        .absolute()
                        .left(px(0.))
                        .top(px(0.))
                        .h(px(4.))
                        .w(gpui::relative(frac))
                        .rounded(px(2.))
                        .bg(accent),
                ),
        )
        .into_any_element()
}

fn color_swatch(e: &Element, style: &Computed) -> AnyElement {
    let color = e
        .attr("value")
        .and_then(crate::value::Color::parse)
        .map(|c| c.to_hsla());
    let mut d = apply(div(), style)
        .w(px(28.))
        .h(px(16.))
        .rounded(px(3.))
        .border_1()
        .border_color(rgb(BORDER));
    if let Some(c) = color {
        d = d.bg(c);
    }
    d.into_any_element()
}

/// Полоса выполнения: `<progress value max>`.
fn progress(e: &Element, style: &Computed) -> AnyElement {
    let num = |name: &str, default: f32| -> f32 {
        e.attr(name).and_then(|v| v.parse().ok()).unwrap_or(default)
    };
    let frac = (num("value", 0.) / num("max", 1.).max(0.0001)).clamp(0., 1.);
    // Заполненную часть полосы `accent-color` красит так же, как флажок.
    let accent: gpui::Hsla = style
        .accent_color
        .map(|c| c.to_hsla())
        .unwrap_or_else(|| rgb(ACCENT).into());
    apply(div(), style)
        .h(px(8.))
        .w_full()
        // Внутри колонки шириной по содержимому `w_full` даёт ноль: у полосы
        // нет собственной ширины, и колонка схлопывается вместе с ней.
        .min_w(px(60.))
        .rounded(px(4.))
        .bg(rgb(FIELD_BG))
        .border_1()
        .border_color(rgb(BORDER))
        .child(
            div()
                .h_full()
                .w(gpui::relative(frac))
                .rounded(px(4.))
                .bg(accent),
        )
        .into_any_element()
}

/// Общая рамка поля ввода.
/// Общая рамка поля ввода.
///
/// Умолчания ставятся ТОЛЬКО там, где документ ничего не сказал: раньше они
/// шли безусловно и затирали фон, рамку, скругление и отступы из CSS.
fn field_box(style: &Computed) -> gpui::Div {
    let mut d = apply(div(), style).flex().items_center().min_h(px(24.));
    if style.background.is_none() && style.gradient.is_none() {
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
