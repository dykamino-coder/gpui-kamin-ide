//! Построение коробки OOF без передачи её внешнему слою.

use crate::animation::animation_live::animated;
use crate::interactive::sticky::sticky_wrap;
use crate::layout::options::RenderOpts;
use crate::layout::positioned::predicates::edge_set;
use crate::layout::writing_mode::vertical_hug;
use crate::render::blocks::inset_holder_box;
use crate::style::computed::{BoxStyle, Display};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div};

pub(super) fn build(
    e: &crate::dom::Element,
    inherited: &BoxStyle,
    opts: &RenderOpts,
    frame: &crate::interactive::sticky::element::StickyCell,
    layer_ok: bool,
    root_box: Option<crate::paint::background::RootMeasurement>,
) -> AnyElement {
    let vcx = opts.cx.vcx();
    let kw_len = |l: Option<Len>| {
        matches!(
            l,
            Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent)
        )
    };
    let positioned_out = matches!(
        e.style.computed().position,
        Some(crate::style::computed::Position::Absolute)
            | Some(crate::style::computed::Position::Fixed)
    );
    // Предел ключевым словом содержимого при АВТОМАТИЧЕСКОМ размере —
    // тот же держатель: css-sizing-3 §fit-content в блочной оси даёт
    // высоту содержимого, и used = min(растяжение краями, содержимое)
    // (`position-absolute-fit-content`: `top: 0; bottom: 0;
    // max-height: fit-content` — 100, а не 200). `paint/apply/` такой
    // предел умеет только при `height` в точках и иначе пропускает.
    // Содержимое выше растяжения держатель не зажмёт — приближение.
    let auto_len = |l: Option<Len>| matches!(l, None | Some(Len::Auto));
    let holder_axis = if positioned_out
        && e.style.computed().writing_mode.is_none()
        && auto_len(e.style.computed().height)
        && kw_len(e.style.computed().max_height)
        && edge_set(e.style.computed().inset.top)
        && edge_set(e.style.computed().inset.bottom)
    {
        Some(true)
    } else if positioned_out
        // Письмо коробки держателю не мешает: размеры
        // ортогонального узла ложатся по СВОЕМУ письму, и у
        // `vertical-rl` `block-size: min-content` — физическая ширина
        // (css-writing-modes-4, Abstract-Physical Mapping). Гейт
        // `vertical.is_none()` ставился, пока размеры
        // транспонировались; без держателя ширина тянулась краями
        // `left/right` во всё окно (`div-{min,max,fit}-content-
        // orthogonal-*`: 13.5 %). Ветка высоты выше гейт сохраняет: в
        // вертикальном письме это строчная ось, её пары не разбирались.
        && auto_len(e.style.computed().width) && kw_len(e.style.computed().max_width)
        && edge_set(e.style.computed().inset.left)
        && edge_set(e.style.computed().inset.right)
    {
        Some(false)
    } else {
        None
    };
    // Липкая коробка сдвигает своё поддерево на подготовке: щупы внутри
    // вычитают сдвиг (`ShiftLink`), поэтому звено заводится ДО сборки.
    let sticky_link = (e.style.computed().position
        == Some(crate::style::computed::Position::Sticky))
    .then(|| crate::interactive::scroll_extent::ShiftLink::inside(opts.cx.sticky.as_ref()));
    let sticky_opts = sticky_link
        .as_ref()
        .map(|link| opts.inner(|cx| cx.sticky = Some(link.clone())));
    let opts = sticky_opts.as_deref().unwrap_or(opts);
    let built = if let Some(block_axis) = holder_axis {
        inset_holder_box(e, block_axis, inherited, opts, &kw_len)
    } else {
        crate::paint::effects::grouped::block_group(animated(e, inherited, opts), e, inherited, vcx)
    };
    let built = match root_box {
        Some(area) if e.style.computed().root_box && e.style.computed().canvas_bg => {
            crate::paint::background::record_root_box(
                built,
                area,
                &e.style.computed(),
                opts.base_size(),
                opts.cx.vcx(),
            )
        }
        _ => built,
    };
    let built = vertical_hug(built, e, inherited);
    let built = sticky_wrap(built, &e.style.computed(), frame, layer_ok, sticky_link);
    // Таблица сжимается по содержимому (§17.5.2.2), и выражено это у
    // нас гибким рядом. В контейнере с БЛОЧНОЙ раскладкой гибкого
    // ряда нет, `align_self` мёртв, и таблица растягивалась на всю
    // ширину родителя — видно на `<span style="display:block">` с
    // табличными детьми.
    let table_child = e.tag == "table"
        || matches!(
            e.style.computed().display,
            Some(Display::Table) | Some(Display::InlineTable)
        );
    let block_parent = matches!(
        inherited.display,
        Some(Display::Block) | Some(Display::ListItem) | Some(Display::TableCell)
    );
    if table_child && block_parent && e.style.computed().width.is_none() {
        div().flex().flex_row().child(built).into_any_element()
    } else {
        built
    }
}
