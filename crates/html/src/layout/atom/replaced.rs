//! Замещаемые атомы по тегу: контейнеры, img, svg, canvas.

use crate::dom::Element;
use crate::layout::multicol::spanner::multicol_container;
use crate::layout::options::RenderOpts;
use crate::layout::replaced::image::pct_height_to_px;
use crate::layout::replaced::limits::{atom_base_font, canvas_limit_keywords, with_inherited_font};
use crate::layout::replaced::replaced_content::svg_replaced;
use crate::layout::replaced::replaced_used_style;
use crate::layout::table::table;
use crate::style::cascade::materialize::box_style;
use crate::style::computed::{Align, BoxStyle, Computed, Display};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div};

pub(super) fn container_atom(
    inherited: &BoxStyle,
    opts: &RenderOpts,
    e: &Element,
) -> Option<Option<AnyElement>> {
    let vcx = opts.cx.vcx();
    if e.style.computed().display == Some(Display::GridLanes) {
        return Some(Some(opts.cx.contracts.build.element(e, inherited, opts)));
    }
    if multicol_container(&e.style.computed()) {
        let merged = box_style(inherited, &e.style, vcx);
        let gap = match e.style.computed().column_gap {
            Some(Len::Px(v)) => v,
            _ => match merged.font_size {
                Some(Len::Px(size)) => size,
                _ => opts.base_size(),
            },
        };
        let shrink_to_fit = match (
            e.style.computed().column_count,
            e.style.computed().column_width,
        ) {
            (Some(c), Some(Len::Px(w))) if w > 0.0 => Some(c as f32 * w + (c as f32 - 1.0) * gap),
            _ => None,
        };
        if e.style.computed().width.is_some() || shrink_to_fit.is_some() {
            let mut copy = e.clone();
            if copy.style.computed().width.is_none() {
                copy.style.used_mut().width = shrink_to_fit.map(Len::Px);
            }
            return Some(Some(
                opts.cx.contracts.build.element(&copy, inherited, opts),
            ));
        }
    }
    // Таблица в строке — атомарная коробка со своей табличной раскладкой:
    // путь блока строил бы детей-ряды как обычные блоки, без решётки.
    if e.style.computed().display == Some(Display::InlineTable) {
        let built = table(e, &box_style(inherited, &e.style, vcx), opts);
        // `vertical-align` коробки в строке: низ/верх/середина СТРОКИ, а не
        // базовая линия. Строка — гибкий ряд, и место коробки задаёт её
        // собственный `align-self`.
        let self_align = match e.style.own_vertical_align() {
            Some(Align::End) => Some(gpui::AlignItems::FlexEnd),
            Some(Align::Start) => Some(gpui::AlignItems::FlexStart),
            Some(Align::Center) => Some(gpui::AlignItems::Center),
            _ => None,
        };
        if let Some(a) = self_align {
            let mut wrap = div().flex_shrink_0();
            wrap.style().align_self = Some(a);
            return Some(Some(wrap.child(built).into_any_element()));
        }
        return Some(Some(built));
    }
    None
}

pub(super) fn img_atom(inherited: &BoxStyle, opts: &RenderOpts, e: &Element) -> Option<AnyElement> {
    let vcx = opts.cx.vcx();
    let mut copy = with_inherited_font(&pct_height_to_px(e, inherited), inherited);
    replaced_used_style::inline_percentage_width(
        copy.style.used_mut(),
        opts.cx.constraints.avail_w,
    );
    // Держатель картинки строится из СЫРОГО стиля (`image_with` →
    // `styled_div`), а признак определённого блока (CSS 2.1 §10.5)
    // ставит только `style::cascade::inherit`: без переноса `apply` выбрасывал
    // `height: %` у картинки во flex-элементе, абсолюте, по цепочке
    // долей, и она рисовалась природным размером
    // (`intrinsic-percent-replaced-024/026`: 200×200 вместо 100×100).
    // Ячейка исключена: её высоту считает табличная раскладка движка;
    // лунки — тоже свой путь (`row-auto-repeat-auto-023/024` ломаются с
    // переносом признака).
    if !matches!(
        inherited.display,
        Some(Display::TableCell) | Some(Display::GridLanes)
    ) {
        copy.style.used_mut().cb_height_def_hint =
            box_style(inherited, &e.style, vcx).cb_height_def;
    }
    // Единицы окна (`vw`/`vh`, в том числе внутри `calc`) — в точки: держатель
    // строится из СЫРОГО стиля, а сворачивает их только слитый
    // (`Computed::resolve_viewport`), и `height: calc(60vh - 6px)` у
    // картинки падал в природный размер (`intrinsic-percent-replaced-009-ref`).
    if copy.style.computed().has_viewport_lengths(vcx) {
        copy.style.used_mut().resolve_viewport(opts.viewport, vcx);
    }
    if let Some(doc) = crate::layout::replaced::svg_document_image::render(&copy, opts) {
        return Some(doc);
    }
    Some(
        opts.cx
            .contracts
            .build
            .replaced(&copy, Some(atom_base_font(inherited, opts)), &opts.cx),
    )
}

pub(super) fn svg_atom(inherited: &Computed, opts: &RenderOpts, e: &Element) -> Option<AnyElement> {
    let vcx = opts.cx.vcx();
    // Рисунок без собственного размера — stretch-fit от содержащего
    // блока (`svg::stretch_fit`): ширина родителя, когда она в
    // точках, иначе ближайшая известная (`BoxConstraints::cb_width` — та же, что у
    // картинки с одним соотношением в `image_with`).
    let cb_w = match inherited.width {
        Some(Len::Px(v)) if v > 0.0 => Some(v),
        _ => opts.cx.constraints.cb_width.filter(|v| *v > 0.0),
    };
    // Размер в единицах шрифта (`svg { width: 10ch }`) решается тем же
    // шагом, что у картинки (`image_with` → `resolve_em`): сырой стиль
    // узла несёт `Len::Ch`, а `svg_size::size_of` понимает только точки —
    // рисунок молча падал в размер по `viewBox` (`ch-unit-001-ref`:
    // 150×150 вместо 10ch).
    let mut sized = with_inherited_font(e, inherited);
    sized
        .style
        .used_mut()
        .resolve_em(atom_base_font(inherited, opts), vcx);
    // CSS-коробка `<svg>` (рамка, отбивка) — `svg_replaced`; стиль
    // коробки — свой, с решёнными шрифтовыми единицами.
    let fitted = crate::layout::replaced::svg_size::stretch_fit(&sized, cb_w);
    let shell = sized.style.clone();
    svg_replaced(&sized, &fitted, &BoxStyle::shared(shell.computed()), opts).or_else(|| {
        Some(opts.cx.contracts.build.replaced(
            &with_inherited_font(e, inherited),
            Some(atom_base_font(inherited, opts)),
            &opts.cx,
        ))
    })
}

pub(super) fn canvas_atom(
    inherited: &BoxStyle,
    e: &Element,
    cx: &crate::layout::cx::LayoutCx,
) -> Option<AnyElement> {
    let vcx = cx.vcx();
    // Доля высоты холста — в точки от содержащего блока, ровно как у
    // строчной картинки (`pct_height_to_px`): держатель стоит в
    // анонимном ряду строки с `height: auto`, и доля от ряда
    // схлопывала холст в ноль — флоат выходил нулевой ширины
    // (`intrinsic-percent-replaced-001`: 2.08). Точки берутся только
    // при высоте блока в `Px` и блочном `display` (оговорки там же).
    let converted = pct_height_to_px(&canvas_limit_keywords(e), inherited);
    let e = &converted;
    let merged = box_style(inherited, &e.style, vcx);
    let d = cx
        .contracts
        .build
        .styled_box(e, &merged, cx)
        .flex_shrink_0();
    // Перенос размера через соотношение сторон (css-sizing-4 §4.1)
    // у АТОМАРНОЙ строчной коробки срабатывает лишь тогда, когда
    // соотношение несёт ВНУТРЕННЯЯ коробка, заполняющая названную
    // автором ось: ровно так собран `<img>` (`image_with`), и ровно поэтому
    // `<img style="height:100%">` во флоате определённой высоты
    // выходит квадратом, а холст — полоской. Соотношение на самой
    // коробке этого не даёт (проба руками:
    // 2.08 против 0.00 у той же коробки с `display: block`).
    // Наполнитель ставится только при ОДНОЙ названной оси: при обеих
    // названных соотношение по спеке не действует вовсе
    // (css-sizing-4 §4.1, замечание про automatic size).
    // Под обособлением размера холст меряется как пустой
    // (css-contain-2 §size containment) — там наполнителя быть не
    // должно, иначе он вернул бы размер, который обособление сняло.
    let ratio = e.style.computed().aspect_ratio.filter(|r| {
        r.is_finite()
            && *r > 0.0
            && !e.style.computed().contains_width()
            && !e.style.computed().contains_height()
    });
    let auto = |l: Option<Len>| matches!(l, None | Some(Len::Auto));
    let named = |l: Option<Len>| matches!(l, Some(Len::Px(_)) | Some(Len::Pct(_)));
    // Доля ШИРИНЫ во вкладе в размер контейнера цикличная и считается
    // `auto` (css-sizing-3 §5.2.1: «treated for the purpose of
    // calculating the box's max-content contributions only as that
    // property's initial value»): ширину вклада даёт высота через
    // соотношение, а в раскладке держатель берёт свою долю
    // (`intrinsic-percent-replaced-019/031/032`: `width:100%;
    // height:100%` во flex-элементе высотой 100).
    let pct = |l: Option<Len>| matches!(l, Some(Len::Pct(_)));
    let d = match ratio {
        Some(r)
            if (auto(e.style.computed().width) || pct(e.style.computed().width))
                && named(e.style.computed().height) =>
        {
            let mut fill = div().h(gpui::relative(1.0));
            fill.style().aspect_ratio = Some(r);
            d.child(fill)
        }
        Some(r) if auto(e.style.computed().height) && named(e.style.computed().width) => {
            let mut fill = div().w(gpui::relative(1.0));
            fill.style().aspect_ratio = Some(r);
            d.child(fill)
        }
        _ => d,
    };
    Some(d.into_any_element())
}
