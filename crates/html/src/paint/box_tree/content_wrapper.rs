//! Сборка дерева: обёртка для размера по содержимому (`width`/`height:
//! min-content | max-content | fit-content`). Элемент заворачивается в сетку из
//! одной дорожки нужного вида, и дорожка отдаёт ему ширину содержимого; обёртка
//! сохраняет предпочтительный размер, который видит родитель, несёт отрицательные
//! боковые поля, прижим авто-полями, `align-self` и размещение в сетке родителя.

use crate::defaults::MEDIUM_FONT_SIZE_PX;
use crate::document::state::ValueCx;
use crate::layout::writing_mode::native_intrinsic;
#[path = "content_wrapper/axis_alignment.rs"]
mod axis_alignment;
use crate::dom::Element;
use crate::style::computed::{BoxStyle, Computed};
use crate::style::values::value::Len;
use gpui::{AnyElement, CssSizingKeyword, IntoElement, ParentElement, Styled, div, px};

fn wrapper_width_keyword(style: &Computed) -> Option<CssSizingKeyword> {
    if style.wm().vertical() || style.contains_width() {
        return None;
    }
    match style.width {
        Some(Len::MinContent) => Some(CssSizingKeyword::MinContent),
        Some(Len::MaxContent) => Some(CssSizingKeyword::MaxContent),
        Some(Len::FitContent) if style.fit_arg[0].is_none() => Some(CssSizingKeyword::FitContent),
        // Functional arguments remain owned by the existing wrapper-width path.
        _ => None,
    }
}

/// Every builder must preserve preferred sizing, including the float-band
/// builder, which constructs its children without passing through `blocks`.
pub(crate) fn for_element(
    el: AnyElement,
    element: &Element,
    parent: &BoxStyle,
    placement: (Option<gpui::GridLocation>, Option<gpui::GridLineNames>),
    vcx: ValueCx<'_>,
) -> AnyElement {
    if crate::layout::box_kind::replaced_tag(element) || native_intrinsic::eligible(element) {
        el
    } else {
        content_sized(
            el,
            &element.style.computed(),
            &crate::style::cascade::materialize::box_style(parent, &element.style, vcx),
            placement,
            vcx,
        )
    }
}

/// Размер по содержимому (`width: min-content`/`max-content`).
///
/// У коробки такого размера раскладка под нами не знает — зато знает такую
/// ДОРОЖКУ СЕТКИ. Элемент заворачивается в сетку из одной дорожки нужного
/// вида: ширину она посчитает по содержимому и отдаст элементу. Обёртка
/// прижата к началу строки, иначе сетка растянула бы её саму на всю ширину
/// родителя и смысл потерялся.
pub(crate) fn content_sized(
    el: AnyElement,
    c: &Computed,
    item_style: &Computed,
    placement: (Option<gpui::GridLocation>, Option<gpui::GridLineNames>),
    vcx: crate::document::state::ValueCx<'_>,
) -> AnyElement {
    let track = |l: Option<Len>| match l {
        Some(Len::MinContent) => Some(gpui::GridTrack::MinContent),
        Some(Len::MaxContent) => Some(gpui::GridTrack::MaxContent),
        // `fit-content` — дорожка `auto`: она и есть «по содержимому, но не
        // шире доступного».
        Some(Len::FitContent) => Some(gpui::GridTrack::Auto),
        _ => None,
    };
    let (col, row) = (
        (!c.contains_width()).then(|| track(c.width)).flatten(),
        (!c.contains_height()).then(|| track(c.height)).flatten(),
    );
    if col.is_none() && row.is_none() {
        return el;
    }
    // Позиционированный элемент заворачивать нельзя: обёртка стала бы его
    // содержащим блоком, и края отсчитывались бы от неё. Он и так не
    // растягивается — размер по содержимому получается сам.
    if matches!(
        c.position,
        Some(crate::style::computed::Position::Absolute)
            | Some(crate::style::computed::Position::Fixed)
    ) {
        return el;
    }
    let mut wrap = div().grid();
    if let Some(keyword) = wrapper_width_keyword(c) {
        wrap.style().sizing_keywords = Some([Some(keyword), None]);
    }
    // `fit-content(N)` (css-sizing-3 §4.1): «the fit-content formula with the
    // available space replaced by the specified argument» —
    // min(max-content, max(min-content, N)). Дорожка `auto` считает ровно эту
    // формулу от ширины сетки, значит довод и есть ширина обёртки. Доля — от
    // родителя: при неопределённой базе (вклад в `min-content`/`max-content`)
    // taffy её не решает и меряет обёртку содержимым, как велит §5.2.1
    // (`fit-content-length-percentage-011/012/014`). Это НЕ пара «max-width =
    // N» из отката в `value.rs`: там довод был потолком, здесь — место.
    if matches!(c.width, Some(Len::FitContent)) && !c.wm().vertical() {
        match c.fit_arg[0] {
            Some(Len::Px(v)) => wrap = wrap.w(px(v)),
            Some(Len::Pct(k)) => wrap = wrap.w(gpui::relative(k)),
            _ => {}
        }
    }
    // Боковые поля сняты с элемента вызывающей стороной (дорожка сетки их не
    // считает) — здесь они ставятся на саму обёртку, без лишней коробки:
    // отдельный держатель менял раскладку соседей
    // (`text-transform-fullwidth-008`).
    let side = |l: Option<Len>| -> Option<f32> {
        let size = match c.font_size {
            Some(Len::Px(v)) => v,
            _ => MEDIUM_FONT_SIZE_PX,
        };
        let family = c.font_family.clone().unwrap_or_default();
        match l {
            Some(Len::Px(_) | Len::Em(_) | Len::Ch(_) | Len::Ex(_)) => {
                let v = crate::text::metrics::spacing_px(l, &family, size, vcx);
                (v < 0.0).then_some(v)
            }
            _ => None,
        }
    };
    if let Some(v) = side(c.margin.left) {
        wrap = wrap.ml(px(v));
    }
    if let Some(v) = side(c.margin.right) {
        wrap = wrap.mr(px(v));
    }
    // Intrinsic widths keep their shrink-wrapped box; a height-only wrapper
    // must still let an automatic block width fill the available track.
    // Авто-поля прижимают КОРОБКУ в свободном месте (CSS 2.1 §10.3.3):
    // `margin-left: auto` при точечном max-content — прижим вправо
    // (align-baseline-ref: правый столбец текста уезжал влево).
    let auto = |l: Option<Len>| matches!(l, Some(Len::Auto));
    wrap.style().justify_items = Some(if axis_alignment::fills_width(c) {
        gpui::AlignItems::Stretch
    } else {
        match (auto(c.margin.left), auto(c.margin.right)) {
            (true, false) => gpui::AlignItems::FlexEnd,
            (true, true) => gpui::AlignItems::Center,
            _ => gpui::AlignItems::FlexStart,
        }
    });
    // Прижим внутри дорожки ничего не двигает, когда дорожка `max-content`
    // ровно по коробке, а сама обёртка растянута на строку родителя:
    // свободное место — между ДОРОЖКОЙ и краем сетки. Его делит
    // `justify-content` (css-grid-1 §10.5); без него `margin-left: auto;
    // width: max-content` оставался у левого края (`align-baseline-ref`).
    match (auto(c.margin.left), auto(c.margin.right)) {
        (true, false) => wrap.style().justify_content = Some(gpui::AlignContent::FlexEnd),
        (true, true) => wrap.style().justify_content = Some(gpui::AlignContent::Center),
        _ => {}
    }
    // Выравнивание элемента поперёк РОДИТЕЛЯ переезжает на обёртку: во
    // флексе родителя стоит она, и без переноса `justify-items: center` в
    // лунках глох на min-content-элементах
    // (column-fill-reverse-justify-items-001). В вертикальном письме
    // align-self несёт ось самого движка — перенос ломал ортогональные
    // потоки (three-levels-of-orthogonal-flows).
    if let Some(a) = c.align_self.filter(|_| !c.wm().vertical()) {
        // `anchor-center` без якоря ведёт себя как `center` (css-anchor-position-1 §5.2).
        wrap.style().align_self = Some(crate::paint::apply::self_align(a, c.align_self_last));
    }
    if let Some(col) = col {
        wrap = wrap.grid_template_cols(vec![col]);
    }
    if let Some(row) = row {
        wrap = wrap.grid_template_rows(vec![row]);
    }
    crate::paint::apply::item_metadata::project_wrapper(wrap.style(), item_style);
    // Размещение элемента в сетке родителя — на обёртке (см. вызов).
    let (location, names) = placement;
    if let Some(location) = location {
        wrap.style().grid_location = Some(location);
    }
    if let Some(names) = names {
        wrap.style().grid_line_names = Some(Box::new(names));
    }
    wrap.child(el).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn intrinsic_wrapper_preserves_keyword_without_duplicating_fit_argument() {
        let mut style = Computed {
            width: Some(Len::FitContent),
            ..Computed::default()
        };
        assert_eq!(
            wrapper_width_keyword(&style),
            Some(CssSizingKeyword::FitContent)
        );
        for argument in [Len::Px(50.0), Len::Pct(0.5)] {
            style.fit_arg[0] = Some(argument);
            assert_eq!(wrapper_width_keyword(&style), None);
        }
        style.fit_arg[0] = None;
        style.width = Some(Len::MinContent);
        assert_eq!(
            wrapper_width_keyword(&style),
            Some(CssSizingKeyword::MinContent)
        );
        style.width = Some(Len::MaxContent);
        assert_eq!(
            wrapper_width_keyword(&style),
            Some(CssSizingKeyword::MaxContent)
        );
        style.writing_mode = Some(crate::style::computed::writing_mode::WritingModeKw::new(
            true,
            style.wm().block_rl(),
            style.wm().sideways(),
        ));
        assert_eq!(wrapper_width_keyword(&style), None);
    }
}
