//! Замещаемый атом на статической позиции как overlay абзаца.

use crate::dom::Element;
use crate::hooks::IntoHooked;
use crate::layout::options::RenderOpts;
use crate::layout::positioned::anchor;
use crate::layout::replaced::image::image;
use crate::paint::box_tree::styled::styled_div_with;
use crate::paint::effects::grouped::grouped;
use crate::render::blocks::blocks;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::inline;
use gpui::{IntoElement, ParentElement, Styled, div};

#[allow(clippy::too_many_arguments)]
pub(crate) fn static_overlay(
    e: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
    rot_block: bool,
) -> Option<inline::Piece> {
    let vcx = opts.cx.vcx();
    let mut merged = inherit(inherited, &e.style, vcx);
    anchor::prepare_inline_abs(&mut merged, e, inherited, vcx);
    // Стиль с позицией — для якорного заместителя (`anchor::place`).
    let positioned = merged.clone();
    let mut group_style = (*e.style).clone();
    group_style.mask_image = merged.mask_image.clone();
    merged.position = None;
    merged.abs_static = true;
    // Замещаемый элемент строит своя ветка: дети `<svg>` — не блоки,
    // путь блоков давал пустую коробку (clip-path-ellipse-2-ref).
    // Картинка — тем же порядком: у неё детей нет вовсе, и путь
    // блоков давал пустую коробку, то есть абсолютная картинка без
    // краёв не рисовалась ВООБЩЕ (`clip-rect-v*`, проба
    // `probe/absimg2.html`).
    let inner = if e.tag == "svg" {
        let mut copy = e.clone();
        copy.style.image_orient_none = merged.image_orient_none;
        copy.style.position = None;
        crate::svg::element_with_html(&copy, opts).unwrap_or_else(|| image(&copy, &opts.cx))
    } else if e.tag == "img" {
        let mut copy = e.clone();
        copy.style.image_orient_none = merged.image_orient_none;
        copy.style.position = None;
        // `clip: rect(...)` и маска у картинки живут в буфере группы:
        // путь наложения идёт мимо `grouped`, и без обёртки картинка
        // рисовалась бы целиком (`clip-rect-v*`).
        grouped(image(&copy, &opts.cx), &group_style, vcx)
    } else {
        // Тот же буфер группы, что и у картинки: маска, обрезка и
        // фильтр иначе не доходят до коробки на статической позиции.
        grouped(
            styled_div_with(e, &merged, &opts.cx)
                .children(blocks(&e.children, &merged, opts))
                .into_any_element(),
            &group_style,
            vcx,
        )
    };
    // Transform the actual out-of-flow box before anchoring it; the
    // paragraph overlay has zero advance and is not a transform reference box.
    let inner = crate::paint::effects::transform::transformed(inner, &e.style, inherited, vcx);
    // Сторона, которой коробка вешается на статическую точку. При
    // `direction: ltr` — левый край (текстовый путь так и кладёт,
    // `lines.rs: prepaint_at(origin)`), при `rtl` — ПРАВЫЙ:
    // css-position-3 §abs-non-replaced-width (строки 1035-1043,
    // перепись CSS 2.1 §10.3.7) — «…if the 'direction' property of the
    // element establishing the static-position containing block is
    // 'ltr' set 'left' to the static position …; otherwise, set
    // 'right' to the static-position». Точка у обеих сторон ОДНА И ТА
    // ЖЕ: замер по снимкам — ltr-двойники `-v{lr,rl}-{004,005,028,029,
    // 104,105,136,137}` ставят левый край ровно на 168.0 и все восемь
    // 0.00, а эталон rtl-пар требует 88.0..168.0, то есть ту же 168.0
    // правым краем.
    //
    // Blink разводит это на два шага: `geometry/static_position.h:86`
    // даёт `kInlineEnd` при `!IsLtr()`, а `absolute_utils.cc:27-37`
    // `GetStaticPositionInsetBias` переводит его в `InsetBias::kEnd`.
    // Сторону задаёт направление СОДЕРЖАЩЕГО блока, а не собственное
    // письмо коробки (css-writing-modes-4 §7.1, строки 1926-1931).
    let inner = anchor::place::place(inner, &positioned, inherited, vcx);
    let rotated_rtl = inherited.wm().rtl() && inherited.wm().turned();
    let inner = if inherited.wm().rtl() && !rot_block && !rotated_rtl {
        crate::layout::positioned::containing_block::InlineStartHang::new(inner).hooked()
    } else {
        inner
    };
    // Кусок кладётся КОРНЕМ в статическую точку (`lines.rs`), а корень
    // своих полей не кладёт: от статической позиции коробку отодвигает
    // её поле (CSS 2.1 §10.3.7, `margin-left` в уравнении ширины). Под
    // обёрткой коробка — обычный ребёнок, и поле на месте
    // (`CSS2/text/text-indent-013-ref`: `margin-left: -10em` — чёрная
    // полоса на 328 вместо 168 закрывала PASS).
    let nz = |l: Option<Len>| matches!(l, Some(Len::Px(v)) if v.abs() > 0.001);
    let inner = if nz(merged.margin.left) || nz(merged.margin.top) {
        div()
            .flex()
            .flex_row()
            .items_start()
            .child(inner)
            .into_any_element()
    } else {
        inner
    };
    Some(inline::Piece::Overlay(
        inner,
        inline::OverlayAt {
            next_line: rot_block
                || (inherited.band_kid
                    && !crate::layout::positioned::static_position::hypothetical_inline_level(e)),
            bidi_hang: rotated_rtl && !rot_block,
            ..Default::default()
        },
    ))
}
