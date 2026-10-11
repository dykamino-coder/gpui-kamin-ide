//! Градиенты и image-set: разбор, угол, растр, image-resolution.

use crate::style::computed::*;
use crate::style::values::value::{Color, Len};

mod direction;
mod image_set;
mod parse;
use direction::gradient_direction;
pub(super) use image_set::image_set_pick;
use parse::gradient_angle;
pub(crate) use parse::parse_gradient;

/// ★ ЗАМЕРЕНО И ОТКАЧЕНО (09.09, v168, `scout-bgimg-2026-09.md` §4
/// IMG-IMAGE-SET, 2 хунка): выбор кандидата `image-set()` по спеке — отсев по
/// MIME, дублям разрешения и «кандидатов не осталось ⇒ негодная картинка».
/// Обещание +13. Полный свод против v37: **+9/−17**. Плюсы — семья, где
/// кандидат ДОЛЖЕН быть отвергнут (`image-set-type-unsupported-*`,
/// `-zero-resolution-*`, `-negative-resolution-*`, градиенты). Минусы — 17
/// пар с единственным годным кандидатом (`image-set-rendering`, `-dpi-*`,
/// `-dppx-*`, `-calc-x-*`, `-no-res-*`, `-type-*`), все ровно 2.08: картинка
/// встала не на место. Отбор верен, теряется адрес выбранного кандидата —
/// возвращать вместе с разбором `<string>` как адреса (стенд `wptrun.rs:704`
/// перебазирует только `url(...)`).
/// Записи `<gradient>`, которых GPU-путь не выражает: коническая (обход по
/// углу) и все повторяющиеся (узор стопов мостится вдоль линии —
/// css-images-3 §3.6). Такие рисуются растровой плиткой — тем же путём,
/// которым уже ходит `conic-gradient()`.
pub(crate) fn gradient_as_raster(v: &str) -> bool {
    // `cross-fade()` (css-images-4 §2.6) — смесь картинок растром той же
    // плиткой (`background::rasterize_cross_fade`).
    v.starts_with("cross-fade(")
        || v.starts_with("conic-gradient(")
        || v.starts_with("repeating-linear-gradient(")
        || v.starts_with("repeating-radial-gradient(")
        || v.starts_with("repeating-conic-gradient(")
}

/// Пересчитать фильтром цвета стопов в ЗАПИСИ растрового градиента: такие
/// градиенты живут строкой в `bg_image`, а не полем `gradient`. Слова, не
/// являющиеся цветом (направление, позиции, `from`/`at`), остаются как есть.
pub(crate) fn filter_gradient_text(raw: &str, f: &Filter) -> String {
    let (Some(open), Some(close)) = (raw.find('('), raw.rfind(')')) else {
        return raw.to_string();
    };
    if close <= open {
        return raw.to_string();
    }
    let parts: Vec<String> = crate::style::css::split_args(&raw[open + 1..close])
        .into_iter()
        .map(|part| {
            split_outside_parens(part)
                .into_iter()
                .map(|w| match Color::parse(&w) {
                    Some(c) => {
                        let c = f.apply(c);
                        format!(
                            "rgba({},{},{},{})",
                            (c.r * 255.0).round(),
                            (c.g * 255.0).round(),
                            (c.b * 255.0).round(),
                            c.a
                        )
                    }
                    None => w,
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    format!("{}{})", &raw[..=open], parts.join(", "))
}
