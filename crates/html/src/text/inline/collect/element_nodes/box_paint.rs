//! Слитый стиль строчного элемента при сборе кусков: что из коробки элемента рисует прогон
//! текста. Накопленный относительный сдвиг, фон строчной коробки с отбивкой и радиусом
//! (кроме `background-clip: text`), рамка (ровная или с разными гранями), прозрачная уникальная
//! полоса для рамки без фона (CSS 2.1 §8.6, css-break-3 §5.4) и контур `outline`. На выходе
//! `Computed` с `inline_*` полями и признак «фон рисуется».

use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::Computed;
use crate::style::values::value::{Color, Len, px_lossy_zero};
use crate::text::inline::*;

pub(super) fn element_style(
    e: &Element,
    inherited: &Computed,
    vcx: ValueCx<'_>,
) -> (Computed, bool) {
    let mut merged = inherit(inherited, &e.style, vcx);
    merged.self_node = e.node_id;
    let viewport = vcx.viewport;
    merged.resolve_decor_inset_viewport(viewport, vcx);
    // Относительный сдвиг строчного КОПИТСЯ вниз: вложенные
    // куски двигаются на сумму сдвигов предков.
    // Вставки — из СЛИТОГО стиля: там `em` уже в точках (`resolve_em`), а
    // в собственном стиле `left: -6em` читался нулём, и вложенный
    // относительный кусок не сдвигался (`position-relative-033`).
    let own_rel = if e.style.position == Some(crate::style::computed::Position::Relative) {
        let px_of = px_lossy_zero;
        let i = &merged.inset;
        (
            px_of(i.left) - px_of(i.right),
            px_of(i.top) - px_of(i.bottom),
        )
    } else {
        (0.0, 0.0)
    };
    if own_rel != (0.0, 0.0) {
        let base = merged.rel_shift.unwrap_or((0.0, 0.0));
        merged.rel_shift = Some((base.0 + own_rel.0, base.1 + own_rel.1));
    }
    // Фон строчного бокса рисует прогон текста: коробки у него
    // нет, а фон обязан рваться на переносах вместе со строкой.
    // Берётся из СЛИТОГО стиля: отложенный цвет (`currentColor`,
    // относительная функция) решён только там.
    // `background-clip: text`: бокс фоном не красится — заливку
    // несёт цвет глифов (`inherit`), узорную показать нечем.
    if let Some(bg) = merged
        .background
        .filter(|_| merged.bg_clip != Some(crate::style::computed::BgClip::Text))
    {
        merged.inline_bg = Some(bg);
        // Единицы шрифта разрешаются так же, как в `inline_sides`:
        // разбор только по точкам ронял `padding: 1em` в ноль, и
        // подсветка строчной коробки шла впритык к тексту.
        // Проверено: раздувать `line_bounds` в `vendor/gpui/.../line.rs`
        // на перелив полосы прогона, чтобы фон строки не попадал в один
        // порядок с глифами соседней, ничего не меняет.
        let size = match merged.font_size {
            Some(crate::style::values::value::Len::Px(v)) => v,
            _ => 16.0,
        };
        let family = merged.font_family.clone().unwrap_or_default();
        let px_of = |l: Option<crate::style::values::value::Len>| match l {
            Some(
                crate::style::values::value::Len::Px(_)
                | crate::style::values::value::Len::Em(_)
                | crate::style::values::value::Len::Ch(_)
                | crate::style::values::value::Len::Ex(_)
                | crate::style::values::value::Len::Ic(_),
            ) => crate::text::metrics::spacing_px(l, &family, size, vcx),
            _ => 0.0,
        };
        merged.inline_pad = Some(physical_sides::project(
            inherited,
            [
                px_of(e.style.padding.top),
                px_of(e.style.padding.right),
                px_of(e.style.padding.bottom),
                px_of(e.style.padding.left),
            ],
        ));
        merged.inline_radius = Some(px_of(e.style.radius.tl));
    }
    // Рамка строчной коробки рисуется прогоном: и ровная, и
    // с РАЗНЫМИ гранями (у прогона теперь пооосевые ширины) —
    // коробка рвала перенос, и span с рамкой уезжал столбиком.
    let font_px = match merged.font_size {
        Some(Len::Px(v)) => v,
        _ => 16.0,
    };
    let own_bg = merged
        .background
        .filter(|_| merged.bg_clip != Some(crate::style::computed::BgClip::Text))
        .is_some();
    let mut own_border = true;
    if let Some((color, width)) = uniform_border(&e.style, font_px, vcx) {
        merged.inline_border = Some((color, [width; 4]));
    } else if let Some(sided) = sided_border(&e.style, font_px, vcx) {
        merged.inline_border = Some((sided.0, physical_sides::project(inherited, sided.1)));
    } else {
        own_border = false;
    }
    // CSS 2.1 §8.6 / css-break-3 §5.4: an inline box with a border
    // and no background still paints its border (and its padding
    // area) on each fragment. The text run band is the painter, and
    // `vendor/gpui` starts a band only from a background colour, so
    // such a box gets a fully transparent band colour that is unique
    // per box: bands of adjacent boxes stay separate, runs of its
    // descendants (which inherit it) continue the same band.
    // Своя коробка (подложка или рамка) едет СВОИМ накопленным сдвигом.
    if own_bg || own_border {
        merged.box_shift = Some(merged.rel_shift.unwrap_or((0.0, 0.0)));
    }
    let mut painted_bg = merged.inline_bg.is_some();
    if own_border && !own_bg && !painted_bg {
        let id = BORDER_BAND.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        merged.inline_bg = Some(Color {
            r: (id % 251) as f32 / 251.0,
            g: ((id / 251) % 251) as f32 / 251.0,
            b: 0.5,
            a: 0.0,
        });
        let family = merged.font_family.clone().unwrap_or_default();
        let px_of = |l: Option<crate::style::values::value::Len>| match l {
            Some(
                crate::style::values::value::Len::Px(_)
                | crate::style::values::value::Len::Em(_)
                | crate::style::values::value::Len::Ch(_)
                | crate::style::values::value::Len::Ex(_)
                | crate::style::values::value::Len::Ic(_),
            ) => crate::text::metrics::spacing_px(l, &family, font_px, vcx),
            _ => 0.0,
        };
        merged.inline_pad = Some(physical_sides::project(
            inherited,
            [
                px_of(e.style.padding.top),
                px_of(e.style.padding.right),
                px_of(e.style.padding.bottom),
                px_of(e.style.padding.left),
            ],
        ));
        merged.inline_radius = Some(px_of(e.style.radius.tl));
    }
    // Контур строчного куска рисует тот же прогон: коробки у
    // куска нет, а место контур и не занимает. Рисуется только
    // ЗАДАННЫЙ видимый стиль (начальное `outline-style` — `none`),
    // толщина без записи — `medium`, цвет без своего — цвет текста
    // куска С НАСЛЕДОВАНИЕМ (`merged`, а не свой `e.style`), без
    // него — чёрный. Полосу прогона `paint_line_background`
    // заводит ТОЛЬКО от фона (`line.rs`, `current_background`),
    // поэтому контуру без фона даётся прозрачная подложка: иначе
    // кольцо не рисовалось вовсе (`outline-004`, `outline-022`).
    // Не заводить полосу в `line.rs` от ЛЮБОЙ рамки: обычная рамка
    // тогда рисуется дважды (`bidi-00*`); здесь — только контур,
    // второго рисовальщика у которого нет.
    if merged.inline_border.is_none()
        && e.style.display.is_none()
        && let Some(o) = e.style.outline.as_ref()
        && o.style.is_some_and(|s| s != 0)
        && let Some(width) = match o.width {
            Some(Len::Px(w)) => Some(w),
            None => Some(3.0),
            _ => None,
        }
        && width > 0.0
    {
        let color = merged.used_outline_color(o);
        merged.inline_border = Some((color, [width; 4]));
        if merged.inline_bg.is_none() {
            merged.inline_bg = Some(Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            });
            painted_bg = true;
        }
    }
    // Атомарная строчная коробка — ГРАНИЦА переноса, даже когда
    // своей коробки в раскладке ей не завели (размер не задан, и
    // она осталась куском текста). По CSS строку можно рвать
    // между текстом и такой коробкой; у нас это выражается
    // нулевым пробелом по краям (`line-breaking-atomic-007`).
    // Настоящий `display: inline` сюда НЕ входит: разбор держит
    // его как `InlineBlock` с пометкой `inline_display`, и без
    // отсечки каждый `<span>` обрамлялся служебными нулевыми
    // пробелами — они рвали ряд схлопываемых пробелов, возвращали
    // краевой проход и добавляли лишнюю точку переноса. Та же
    // отсечка стоит в `has_own_box`.
    (merged, painted_bg)
}
