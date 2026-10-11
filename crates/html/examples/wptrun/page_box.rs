//! Print-pair page box: `@page` declarations resolved into sheet geometry.

use super::page_box_values::{
    abs_len, apply_border, apply_margin, apply_padding, apply_size, default_margin, fit,
    logical_side, page_font_size, rel_len,
};

/// Лист по номеру и имени страницы.
pub(super) type PageBoxFn = std::rc::Rc<dyn Fn(usize, &str) -> PageBox>;

/// Вычисленная коробка страницы печатной пары.
pub(super) struct PageBox {
    /// Полный размер листа.
    pub(super) size: (f32, f32),
    /// Поля: верх/право/низ/лево.
    pub(super) margin: [f32; 4],
    /// Отступы листа: верх/право/низ/лево. Page area — КОНТЕНТНАЯ область
    /// коробки страницы, внутри отступов (css-page-3 §page-model; Blink
    /// `ResolvePageBoxGeometry` считает их как у обычного блока).
    pub(super) padding: [f32; 4],
    /// Область содержимого (page area).
    pub(super) area: (f32, f32),
    pub(super) bg: Option<kamin_html::value::Color>,
    pub(super) border: (f32, kamin_html::value::Color),
    /// Контур листа: толщина, сдвиг, цвет (`outline`/`outline-offset`).
    pub(super) outline: (f32, f32, kamin_html::value::Color),
    /// `page-orientation`: 0 — upright, 1 — rotate-right, 3 — rotate-left.
    pub(super) turn: u8,
}

pub(super) fn page_box(
    decls: Vec<(String, String)>,
    root_margin: [f32; 4],
    wm: (bool, bool, bool),
) -> PageBox {
    use kamin_html::value::Color;
    let (mut w, mut h) = (480.0f32, 288.0f32);
    let mut margin = [default_margin(); 4];
    let mut padding = [0.0f32; 4];
    // Стороны с `margin: auto` — их размер решается остатком (ниже).
    let mut auto_m = [false; 4];
    let logical = move |block: bool, start: bool| -> usize { logical_side(wm, block, start) };
    // `width`/`height` листа — размер PAGE AREA, не листа (css-page-3
    // §page-model); применяются после полей, см. ниже.
    let (mut explicit_w, mut explicit_h): (Option<f32>, Option<f32>) = (None, None);
    let mut bg: Option<Color> = None;
    // Контур по умолчанию — `currentColor`, у листа это чёрный.
    let mut outline = (
        0.0f32,
        0.0f32,
        Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
    );
    let mut border = (
        0.0f32,
        Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
    );
    // Длина: точки как есть; vw/vh — от ДЕФОЛТНОЙ страницы (по тестам
    // page-size-016/017); проценты полей — от ОБЪЯВЛЕННОГО размера
    // страницы по своей оси (page-margin-005: 10% от 300px = 30px),
    // поэтому размер считается первым проходом.
    let fs = page_font_size(&decls);
    let px_abs = |t: &str| -> Option<f32> { abs_len(t, fs) };
    for (k, v) in &decls {
        match k.as_str() {
            "size" => apply_size(v, fs, &mut w, &mut h),
            "width" => explicit_w = px_abs(v).or(explicit_w),
            "height" => explicit_h = px_abs(v).or(explicit_h),
            _ => {}
        }
    }
    let (pw, ph) = (w, h);
    let mut turn = 0u8;
    let px_of = move |t: &str, axis_h: bool| -> Option<f32> { rel_len(t, axis_h, fs, (pw, ph)) };
    for (k, v) in &decls {
        match k.as_str() {
            "margin" if v.trim() == "inherit" => {
                // `page-margin-006`: `margin: 13px; margin: inherit` → поля
                // корневого элемента (0.5in), не 13px.
                margin = root_margin;
            }
            "margin" => apply_margin(v, px_of, &mut margin, &mut auto_m),
            "margin-top" | "margin-right" | "margin-bottom" | "margin-left" => {
                let i = match k.as_str() {
                    "margin-top" => 0,
                    "margin-right" => 1,
                    "margin-bottom" => 2,
                    _ => 3,
                };
                auto_m[i] = v.trim() == "auto";
                if auto_m[i] {
                    margin[i] = 0.0;
                } else if let Some(px) = px_of(v, i % 2 == 0) {
                    margin[i] = px;
                }
            }
            // Отступы листа. Проценты — по СВОЕЙ физической оси, как у полей
            // (css-page-3 §page-properties: «for right and left values,
            // percentages are relative to the width of the containing block;
            // for top and bottom values, … to the height»): `page-box-007`
            // `5% 20% 15% 40%` от 400x800 = 40/80/120/160.
            "padding" => apply_padding(v, px_of, &mut padding),
            "padding-top" => padding[0] = px_of(v, true).unwrap_or(padding[0]),
            "padding-right" => padding[1] = px_of(v, false).unwrap_or(padding[1]),
            "padding-bottom" => padding[2] = px_of(v, true).unwrap_or(padding[2]),
            "padding-left" => padding[3] = px_of(v, false).unwrap_or(padding[3]),
            // Логические поля и отступы (`margin-block-start` и родня): сторона
            // — по письму корня, доля — по физической оси этой стороны.
            k if (k.starts_with("margin-") || k.starts_with("padding-"))
                && (k.contains("-block-") || k.contains("-inline-"))
                && (k.ends_with("-start") || k.ends_with("-end")) =>
            {
                let i = logical(k.contains("-block-"), k.ends_with("-start"));
                let slot = if k.starts_with("padding-") {
                    &mut padding[i]
                } else {
                    &mut margin[i]
                };
                if let Some(px) = px_of(v, i % 2 == 0) {
                    *slot = px;
                }
            }
            "background" | "background-color" => {
                let first = v.split_whitespace().next().unwrap_or("");
                if let Some(c) = Color::parse(first) {
                    bg = Some(c);
                }
            }
            "outline" => {
                for t in v.split_whitespace() {
                    if let Some(px) = px_of(t, false) {
                        outline.0 = px;
                    } else if let Some(c) = Color::parse(t) {
                        outline.2 = c;
                    }
                }
            }
            // css-page-3 §page-orientation-prop: лист раскладывается как
            // обычно и ПОКАЗЫВАЕТСЯ повёрнутым на четверть оборота.
            "page-orientation" => {
                turn = match v.trim().to_ascii_lowercase().as_str() {
                    "rotate-right" => 1,
                    "rotate-left" => 3,
                    _ => 0,
                }
            }
            "outline-width" => outline.0 = px_of(v, false).unwrap_or(outline.0),
            "outline-offset" => outline.1 = px_of(v, false).unwrap_or(outline.1),
            "outline-color" => outline.2 = Color::parse(v.trim()).unwrap_or(outline.2),
            "border" => apply_border(v, px_of, &mut border),
            _ => {}
        }
    }
    // css-page-3 §page-model: при переопределении «instead of ignoring any
    // margins, the containing block is resized to coincide with the margin
    // edges of the page box» (Blink `ResolvePageBoxGeometry`);
    // `page-size-013`: size 500px, margin 50px, width 200px, height 300px →
    // лист 300x400 (эталон `size: 300px 400px; margin: 50px`).
    // `width`/`height` — контентная коробка листа, то есть page area: лист =
    // она + отступы + рамка + поля.
    // Ось с `auto`-полем: лист остаётся размером `size`, а `auto`-поля делят
    // остаток — он бывает ОТРИЦАТЕЛЬНЫМ, если коробка шире `size` (Blink
    // `ResolvePageBoxGeometry` → `ResolveAutoMargins`, csswg#8508;
    // `page-margin-auto-negative-print.tentative`: size 300, width 340,
    // margin auto → по −20 с каждой стороны). Ось без `auto` — прежнее
    // переопределение: лист подгоняется под поля (`page-size-013`).
    let [mt, mr, mb, ml] = &mut margin;
    if let Some(x) = explicit_w {
        let used = x + border.0 * 2.0 + padding[1] + padding[3];
        w = fit(w, used, auto_m[3], auto_m[1], ml, mr);
    }
    if let Some(y) = explicit_h {
        let used = y + border.0 * 2.0 + padding[0] + padding[2];
        h = fit(h, used, auto_m[0], auto_m[2], mt, mb);
    }
    let area = (
        (w - margin[1] - margin[3] - border.0 * 2.0 - padding[1] - padding[3]).max(0.0),
        (h - margin[0] - margin[2] - border.0 * 2.0 - padding[0] - padding[2]).max(0.0),
    );
    // `visibility: hidden` у листа прячет его украшения, не геометрию
    // (`page-visibility-hidden-001-print`: красная рамка листа не видна,
    // содержимое на месте).
    if decls
        .iter()
        .rev()
        .find(|(k, _)| k == "visibility")
        .is_some_and(|(_, v)| matches!(v.trim(), "hidden" | "collapse"))
    {
        border.1.a = 0.0;
        bg = None;
        outline.0 = 0.0;
    }
    PageBox {
        size: (w, h),
        margin,
        padding,
        area,
        bg,
        border,
        outline,
        turn,
    }
}
