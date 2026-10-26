//! Абсолютный строчный атом с обеими свободными осями.

use crate::dom::Element;
use crate::layout::build_contracts::Effect;
use crate::layout::options::RenderOpts;
use crate::layout::positioned::anchor;
use crate::layout::replaced::replaced_content::svg_replaced;
use crate::style::cascade::materialize::box_style;
use crate::style::computed::BoxStyle;
use crate::text::text_box::line_height_px;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div};

pub(in crate::layout::atom) fn static_position_atom(
    inherited: &BoxStyle,
    e: &Element,
    opts: &RenderOpts,
) -> Option<AnyElement> {
    if let Some(child) = anchor::trials::inline::defer(e, inherited, opts) {
        return Some(child);
    }
    let vcx = opts.cx.vcx();
    let mut merged = box_style(inherited, &e.style, vcx);
    anchor::prepare_inline_abs(&mut merged, e, inherited, &opts.cx);
    let positioned = merged.clone();
    // Позиционирование с внутренней коробки СНИМАЕТСЯ. Содержащим блоком
    // ей стала бы нулевая пустышка, а ширина у неё «по содержимому» —
    // в нулевом блоке это ноль, и элемент пропадал вовсе. Без
    // позиционирования она меряется своим содержимым и висит от угла
    // пустышки, то есть ровно от статической позиции.
    merged.position = None;
    merged.abs_static = true;
    // Замещаемый элемент строит своя ветка: голая коробка со стилем
    // теряла содержимое (сломанная картинка с alt-подписью пропадала,
    // `abs-pos-vlr-border-001`). Позиция снимается копией — та же
    // причина, что и у merged ниже.
    let replaced: Option<AnyElement> = match e.tag.as_str() {
        "img" | "svg" => {
            let mut copy = e.clone();
            copy.style.used_mut().position = None;
            // `image-orientation` НАСЛЕДУЕТСЯ (css-images-3 §5.4), а
            // копия несёт только собственный стиль элемента: без этой
            // строки `image-orientation: none`, заданный на `body`, до
            // картинки не доезжает и EXIF-разворот применяется всё равно.
            copy.style.used_mut().image_orient_none = merged.image_orient_none;
            Some(match copy.tag.as_str() {
                // CSS-коробка `<svg>` (рамка, отбивка) — `svg_replaced`;
                // позиция снята и со слитого стиля, как с копии выше.
                "svg" => {
                    let mut unpositioned = merged.clone();
                    unpositioned.position = None;
                    svg_replaced(&copy, &copy, &unpositioned, opts)
                        .unwrap_or_else(|| opts.cx.contracts.build.replaced(&copy, None, &opts.cx))
                }
                _ => opts.cx.contracts.build.replaced(&copy, None, &opts.cx),
            })
        }
        _ => None,
    };
    let inner = opts.cx.contracts.build.styled_box(e, &merged, &opts.cx);
    let inner = inner.children(super::children::children(e, &positioned, opts));
    // Пустышка прижимается к ВЕРХУ строки: иначе она садится на базовую
    // линию, и содержимое уезжает под неё — абсолютный блок оказывался
    // ниже своей строки, а не на её месте (видно на `static-position`:
    // зелёный блок висел под коробкой, красное проступало).
    // Рисуется элемент ПОВЕРХ соседей по строке, поэтому содержимое
    // уходит в верхний слой блока-контейнера, а в строке остаётся щуп: он
    // и держит место, и сообщает, куда потом вернуть содержимое.
    let spot: crate::layout::positioned::containing_block::SpotCell = Default::default();
    let below = e.style.used().z_index.is_some_and(|z| z < 0);
    // Блочный элемент встал бы на НОВУЮ строку — там его статическая
    // позиция и находится: левый край содержимого родителя, верх — низ
    // текущей строки. Строчный остаётся точкой в самой строке.
    let inline_level = crate::layout::positioned::static_position::hypothetical_inline_level(e);
    // Флаги направления нужны и СТРОЧНОМУ атому: в rtl-строке статическая
    // позиция — правый край, заместитель вешается правым краем на точку
    // распорки. Начало новой строки — только у блочного.
    spot.set(crate::layout::positioned::containing_block::Spot {
        hole: None,
        next_line: (!inline_level).then(|| line_height_px(inherited, opts)),
        rtl: inherited.wm().rtl(),
        vertical: inherited.wm().vertical(),
        vertical_rl: inherited.wm().block_rl(),
        self_align: if inline_level {
            None
        } else {
            crate::layout::positioned::static_position::inline_static_self_align(e, inherited)
        },
        line_rect: true,
        ..Default::default()
    });
    if let Some(recorder) = &opts.cx.position.geometry_record {
        recorder.spot(
            e.node_id,
            &spot,
            opts.cx.position.line_box.clone(),
            &e.style.used(),
            opts.cx.vcx(),
        );
    }
    let probe = crate::layout::positioned::containing_block::spot_probe(
        spot.clone(),
        false,
        opts.cx.position.line_box.clone(),
        &opts.cx,
    );
    let inner = match replaced {
        Some(el) if merged.corner_shaped() => opts
            .cx
            .contracts
            .build
            .effects(el, Effect::group(&merged, vcx)),
        Some(el) => el,
        None => inner.into_any_element(),
    };
    let inner = crate::layout::positioned::containing_block::metrics::record::wrap(
        e.node_id,
        &positioned,
        inner,
        opts,
    );
    let inner = anchor::place::place_in_context(inner, &positioned, inherited, &opts.cx);
    if anchor::trials::inline::is_root(e, opts) {
        return Some(inner);
    }
    let taken = if below {
        Some(inner)
    } else {
        opts.cx.position.state.late_push(spot, inner)
    };
    return match taken {
        None => Some(probe),
        Some(kept) => {
            let mut hole = div().relative().w_0().h_0().flex_shrink_0();
            hole.style().align_self = Some(gpui::AlignItems::FlexStart);
            Some(hole.child(kept).into_any_element())
        }
    };
}
