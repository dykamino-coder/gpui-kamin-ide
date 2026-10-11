//! apply_layout, этап размеров: природный размер холста, абсолютные, гибкие элементы, width/height/min/max с учётом border-box, aspect-ratio как автоминимум.

use super::*;

pub(super) fn layout_sizing(mut d: Div, c: &Computed, pad_x: f32, pad_y: f32) -> Div {
    // Природный размер холста под порогами — таблица CSS 2.1 §10.4 для
    // замещаемых с соотношением сторон. Обе оси `<canvas>` пришли из
    // атрибутов (`attr_sized`), то есть это natural size, а не заданный
    // автором размер (HTML §4.12.5; холста нет среди «dimension attributes»
    // HTML Rendering §15.3.10): нарушенный порог одной оси переносится на
    // другую через соотношение, а не просто режет свою ось. Раньше
    // `<canvas width=200 height=200 style="max-height: 100px">` выходил
    // 200×100 вместо 100×100 (`percent-height-replaced-in-percent-cell-002`).
    // Пороги и размеры здесь — content-box, отбивки добавит цикл ниже.
    let natural_fit: Option<(f32, f32)> = if c.attr_sized.0
        && c.attr_sized.1
        && let (Some(Len::Px(w)), Some(Len::Px(h))) = (c.width, c.height)
        && w > 0.0
        && h > 0.0
    {
        let px_of = |l: Option<Len>| match l {
            Some(Len::Px(v)) => Some(v),
            _ => None,
        };
        let min_w = px_of(c.min_width).unwrap_or(0.0);
        let min_h = px_of(c.min_height).unwrap_or(0.0);
        // §10.4: «max-width/max-height … less than min-* is treated as min-*».
        let max_w = px_of(c.max_width).unwrap_or(f32::INFINITY).max(min_w);
        let max_h = px_of(c.max_height).unwrap_or(f32::INFINITY).max(min_h);
        let r = w / h;
        let fit = if w > max_w && h > max_h {
            if max_w / w <= max_h / h {
                (max_w, (max_w / r).max(min_h))
            } else {
                ((max_h * r).max(min_w), max_h)
            }
        } else if w < min_w && h < min_h {
            if min_w / w <= min_h / h {
                ((min_h * r).min(max_w), min_h)
            } else {
                (min_w, (min_w / r).min(max_h))
            }
        } else if w < min_w && h > max_h {
            (min_w, max_h)
        } else if w > max_w && h < min_h {
            (max_w, min_h)
        } else if w > max_w {
            (max_w, (max_w / r).max(min_h))
        } else if w < min_w {
            (min_w, (min_w / r).min(max_h))
        } else if h > max_h {
            ((max_h * r).max(min_w), max_h)
        } else if h < min_h {
            ((min_h * r).min(max_w), min_h)
        } else {
            (w, h)
        };
        (fit != (w, h) && fit.0.is_finite() && fit.1.is_finite()).then_some(fit)
    } else {
        None
    };
    // Positioned boxes cannot use the intrinsic grid wrapper: it would change
    // their containing block. Preserve authored keywords for native measurement.
    if matches!(c.position, Some(Position::Absolute) | Some(Position::Fixed)) {
        d.style().sizing_keywords = Some(intrinsic_size::keywords(c));
    }
    // `max-width`/`max-height: min-content | max-content` у ГИБКОГО ЭЛЕМЕНТА
    // (css-sizing-3 §3.2: the keyword «as a maximum size» — the box's
    // min-/max-content size in that axis). Длиной ключевое слово не
    // выражается, и цикл ниже его пропускал: предел терялся вовсе
    // (`flex-item-max-height-min-content`, `flex-item-max-width-min-content`).
    // Раскладка гибкого контейнера меряет его сама (taffy `flexbox.rs`).
    // Только когда предпочтительный размер оси не в точках: такую пару уже
    // переставили (`content_limit_swapped`) или сделали пределом ниже.
    if c.flex_item {
        let kw = |l: Option<Len>| match l {
            Some(Len::MinContent) => Some(gpui::CssSizingKeyword::MinContent),
            Some(Len::MaxContent) => Some(gpui::CssSizingKeyword::MaxContent),
            _ => None,
        };
        let px_size = |l: Option<Len>| matches!(l, Some(Len::Px(_)));
        let keys = [
            kw(c.max_width).filter(|_| !px_size(c.width)),
            kw(c.max_height).filter(|_| !px_size(c.height)),
        ];
        if keys.iter().any(Option::is_some) {
            d.style().max_sizing_keywords = Some(keys);
        }
    }
    for (val, f) in [
        (natural_fit.map(|f| Len::Px(f.0)).or(c.width), 0u8),
        (natural_fit.map(|f| Len::Px(f.1)).or(c.height), 1),
        (c.min_width, 2),
        (c.min_height, 3),
        (c.max_width, 4),
        (c.max_height, 5),
    ] {
        let Some(l) = val else { continue };
        // Размер по содержимому длиной не выражается: его ставит
        // обёртка-сетка (`render::content_sized`). Здесь он обязан
        // ПРОПУСКАТЬСЯ, иначе доходит до общей ветки и становится долей
        // родителя в сто процентов — то есть ровно обратным по смыслу.
        if matches!(
            l,
            Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent
        ) {
            continue;
        }
        let Some(l) = size_percent::resolve(c, l, f) else {
            continue;
        };
        // Доли считаются от родителя и компенсации не требуют.
        let l = match l {
            Len::Px(v) if f % 2 == 0 => Len::Px(v + pad_x),
            Len::Px(v) => Len::Px(v + pad_y),
            other => other,
        };
        // Ключевое слово содержимого в `min-height`/`max-height` (css-sizing-3
        // §4.1): в блочной оси min-content = max-content = высота содержимого,
        // поэтому `min-height: max-content` даёт used = max(H, содержимое),
        // а `max-height: max-content` — min(H, содержимое). Заданная высота
        // становится соответствующим пределом, сама ось — auto
        // (`block-size-with-min-or-max-content-*`).
        let kw = |l: Option<Len>| {
            matches!(
                l,
                Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent)
            )
        };
        if f == 1
            && let Len::Px(h) = l
        {
            if kw(c.min_height) {
                d = d.min_h(px(h));
                continue;
            }
            if kw(c.max_height) {
                d = d.max_h(px(h));
                continue;
            }
        }
        // Смесь «доля ± точки»: поправка `content-box` едет в точечную часть,
        // доля считается от родителя и поправки не требует. Новая пара НЕ
        // кладётся в арену (`calc_store` на каждом кадре раздувал бы её).
        let g = match l {
            Len::Calc(i) => match crate::style::values::value::calc_get(i).pct_px() {
                Some((pct, add)) => {
                    gpui::DefiniteLength::Calc(add + if f % 2 == 0 { pad_x } else { pad_y }, pct)
                }
                None => len_to_gpui(l),
            },
            _ => len_to_gpui(l),
        };
        d = match f {
            0 => d.w(g),
            1 => d.h(g),
            2 => d.min_w(g),
            3 => d.min_h(g),
            4 => d.max_w(g),
            _ => d.max_h(g),
        };
    }
    // Автоминимум по содержимому в ratio-зависимой оси (css-sizing-4 §5.2:
    // «its min-content size capped by its maximum size»): размер из
    // соотношения идёт МИНИМУМОМ этой оси, сам размер остаётся auto — used =
    // max(ratio-размер, содержимое). Отношение в раскладку при этом не
    // отдаётся, иначе taffy зафиксировал бы ось (`block-aspect-ratio-009/…`,
    // `flex-aspect-ratio-040/…`).
    if ratio_as_auto_min(c)
        && let Some(r) = c.aspect_ratio
    {
        // Определённая ось сперва зажимается своими min/max (§5.1 «size
        // transfers»: `block-aspect-ratio-033`); при `box-sizing: border-box`
        // отношение считается по border-box (§5.1), размеры здесь —
        // content-box, поэтому отбивки прибавляются до переноса и
        // вычитаются после (`intrinsic-size-012`).
        let clamp = |v: f32, lo: Option<Len>, hi: Option<Len>| {
            let v = match lo {
                Some(Len::Px(l)) => v.max(l),
                _ => v,
            };
            match hi {
                Some(Len::Px(h)) => v.min(h),
                _ => v,
            }
        };
        let bb = c.border_box == Some(true);
        match (c.width, c.height) {
            (Some(Len::Px(w)), _) => {
                let w = clamp(w, c.min_width, c.max_width);
                let mh = if bb { (w + pad_x) / r - pad_y } else { w / r };
                let mh = clamp(mh.max(0.0), None, c.max_height);
                d = d.min_h(px(mh + pad_y));
                d.style().aspect_ratio_preferred_size = Some([None, Some(mh + pad_y)]);
            }
            (_, Some(Len::Px(h))) => {
                let h = clamp(h, c.min_height, c.max_height);
                let mw = if bb { (h + pad_y) * r - pad_x } else { h * r };
                let mw = clamp(mw.max(0.0), None, c.max_width);
                d = d.min_w(px(mw + pad_x));
                d.style().aspect_ratio_preferred_size = Some([Some(mw + pad_x), None]);
            }
            _ => {}
        }
    }
    d
}
