//! Presentational size for element_style; split out to keep the owning module within 250 lines.

use crate::style::computed::Computed;
use crate::style::values::value::Len;

/// `aspect-ratio: auto && <ratio>` у НЕзамещаемой коробки (css-sizing-4
/// §5.1): «the preferred aspect ratio is the specified ratio … unless it is
/// a replaced element with a natural aspect ratio … size calculations
/// involving the aspect ratio work with the content box dimensions always».
/// У замещаемых запасное соотношение читает отрисовка (`image_with::ratio_of`),
/// здесь их не трогаем. Раскладка движка считает соотношение по
/// border-box при `box-sizing: border-box`, поэтому соотношение контента
/// переводится в соотношение border-box по оси, заданной в точках
/// (`block-aspect-ratio-004/006`, `flex-aspect-ratio-025/026`).
pub(crate) fn promote_auto_ratio(style: &mut Computed, tag: &str) {
    if matches!(
        tag,
        "img"
            | "svg"
            | "canvas"
            | "video"
            | "embed"
            | "object"
            | "iframe"
            | "input"
            | "select"
            | "textarea"
            | "button"
    ) || style.aspect_ratio.is_some()
    {
        return;
    }
    let Some(r) = style
        .aspect_ratio_auto
        .filter(|r| r.is_finite() && *r > 0.0)
    else {
        return;
    };
    if style.border_box != Some(true) {
        style.aspect_ratio = Some(r);
        return;
    }
    let px = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let b = style.borders();
    let pad_x = px(style.padding.left) + px(style.padding.right) + px(b.left) + px(b.right);
    let pad_y = px(style.padding.top) + px(style.padding.bottom) + px(b.top) + px(b.bottom);
    // Ось, заданная в точках (с зажимом своими пределами), либо её предел.
    let axis = |v: Option<Len>, lo: Option<Len>, hi: Option<Len>| -> Option<f32> {
        let clamp = |x: f32| {
            let x = match lo {
                Some(Len::Px(l)) => x.max(l),
                _ => x,
            };
            match hi {
                Some(Len::Px(h)) => x.min(h),
                _ => x,
            }
        };
        match (v, lo) {
            (Some(Len::Px(x)), _) => Some(clamp(x)),
            (_, Some(Len::Px(l))) => Some(l),
            _ => None,
        }
    };
    let w = axis(style.width, style.min_width, style.max_width);
    let h = axis(style.height, style.min_height, style.max_height);
    let border_ratio = match (w, h) {
        (Some(wb), None) => {
            let hb = (wb - pad_x).max(0.0) / r + pad_y;
            (hb > 0.0).then(|| wb / hb)
        }
        (None, Some(hb)) if hb > 0.0 => {
            let wb = (hb - pad_y).max(0.0) * r + pad_x;
            Some(wb / hb)
        }
        _ => None,
    };
    style.aspect_ratio = Some(border_ratio.unwrap_or(r));
}

pub(crate) fn apply_presentational_size(
    style: &mut Computed,
    tag: &str,
    attrs: &[(String, String)],
) {
    if !matches!(
        tag,
        "img" | "canvas" | "embed" | "iframe" | "video" | "object" | "table"
    ) {
        return;
    }
    let value = |name: &str| {
        attrs
            .iter()
            .find(|(k, _)| k == name)
            .and_then(|(_, v)| match v.strip_suffix('%') {
                Some(pct) => pct.trim().parse::<f32>().ok().map(|p| Len::Pct(p / 100.0)),
                None => v.trim().parse::<f32>().ok().map(Len::Px),
            })
    };
    style.attr_width = value("width");
    style.attr_height = value("height");
    // Своего пикселя у холста нет, но размер по умолчанию задан разметкой:
    // 300 на 150 (HTML §4.12.5). Без него `<canvas width="20">` выходил
    // нулевой высоты, а холст без атрибутов — пустым местом.
    // Таблица замещаемой не является: её height уже прошёл каскад намёков
    // (HTML §15.3.8), а `attr_*` держит соотношение сторон замещаемого и
    // таблице не принадлежит. Без этого `<table width="300">` вовсе не
    // доходил до стиля, и таблица сжималась по содержимому.
    if tag == "table" {
        let w = style.attr_width.take();
        style.attr_height = None;
        if matches!(style.width, None | Some(Len::Auto)) {
            style.width = w;
        }
        return;
    }
    // Оба атрибута объявлены разметкой — только тогда природное соотношение
    // сторон холста известно точно. При одном объявленном вторая сторона
    // берётся из умолчания 300/150 ниже, и «соотношением» она быть не может:
    // на заданной атрибутом стороне стоит `flex-basis: content`
    // (`flexbox-flex-basis-content-001a`: `<canvas width="20"
    // style="height: 8px">`).
    let natural_pair = tag == "canvas" && style.attr_width.is_some() && style.attr_height.is_some();
    if tag == "canvas" {
        style.attr_width = style.attr_width.or(Some(Len::Px(300.0)));
        style.attr_height = style.attr_height.or(Some(Len::Px(150.0)));
    }
    if style.width.is_none() {
        style.width = style.attr_width;
        style.attr_sized.0 = tag == "canvas" && style.attr_width.is_some();
    }
    if style.height.is_none() {
        style.height = style.attr_height;
        style.attr_sized.1 = tag == "canvas" && style.attr_height.is_some();
    }
    // Атрибуты холста — ПРИРОДНЫЙ размер, а не заданный автором: HTML §4.12.5
    // («the intrinsic dimensions of the canvas element equal the size of the
    // coordinate space»), и в списке «dimension attributes» HTML Rendering
    // §15.3.10 холста нет. Значит, как только автор назвал в CSS хоть одну
    // ось, оставшаяся обязана прийти из соотношения, а не из атрибута —
    // css-sizing-4 §4.1 «Min/Max Size Transfers» и пример там же: у
    // `<div style="height:100px;float:left"><canvas style="height:100%">`
    // ширина холста и ВКЛАД во внутренний размер равны 100 точкам. Пока
    // атрибут занимал `style.width`, вклад был равен атрибуту, и флоат
    // выходил 10 точек вместо 100 (`intrinsic-percent-replaced-001`).
    // Когда обе оси пришли от атрибутов, это и есть природный размер — там
    // ничего не меняется, и `flex-basis: content`, `contain: size` и спаннер
    // многоколоночника, читающие `attr_width`/`attr_height` отдельно
    // (`render.rs:3140`, `:16979`), работают как прежде.
    if natural_pair && !(style.attr_sized.0 && style.attr_sized.1) {
        if let (Some(Len::Px(w)), Some(Len::Px(h))) = (style.attr_width, style.attr_height)
            && w > 0.0
            && h > 0.0
            && style.aspect_ratio.is_none()
        {
            style.aspect_ratio = Some(w / h);
        }
        if style.attr_sized.0 {
            style.width = None;
        }
        if style.attr_sized.1 {
            style.height = None;
        }
    }
}
