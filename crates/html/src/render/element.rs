//! Узел-элемент в элемент GPUI: element().

mod orthogonal;
mod quirks;
pub(super) use orthogonal::orthogonal_limit;

mod dispatch;
pub(super) use dispatch::dispatch_element;

use crate::dom::Element;
use crate::layout::list::list_item;
use crate::layout::replaced::iframe::{iframe, object_is_document};
use crate::layout::replaced::svg_percentage_size;
use crate::layout::writing_mode::orthogonal_inline;
use crate::paint::effects::mask::PAINT_VIEWPORT;
use crate::paint::effects::transform::transformed;
use crate::render::*;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::Computed;
use gpui::AnyElement;

/// Блочный элемент.
pub(crate) fn element(e: &Element, inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    crate::layout::fragment::paginate::watch::wrap(
        e.node_id,
        element_inner(e, inherited, opts),
        opts,
    )
}

fn element_inner(e: &Element, inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    let vcx = opts.cx.vcx();
    if e.marks.outside_block {
        return crate::layout::float::outside_block::render(e, inherited, opts);
    }
    let svg_sized = svg_percentage_size::resolve(e, inherited);
    let e = svg_sized.as_ref().unwrap_or(e);
    let media_sized =
        crate::layout::replaced::media::resolve(e, inherited, opts.viewport, opts.cx.avail_w, vcx);
    let e = media_sized.as_ref().unwrap_or(e);
    // Высота ряда от внешней колонки — только ЭТОМУ элементу (`FragCx::outer_row`).
    let (outer_row, inner_opts) = opts.take_outer_row();
    let opts = inner_opts.as_ref().unwrap_or(opts);
    let mut merged = inherit(inherited, &e.style, vcx);
    list_item::inherited_style(e, inherited, &mut merged);
    // Якорный шаг: ключи реестров кадра (свой `node_id` для содержащего
    // блока детей, порядок сборки, ключ клетки) и размеры от якоря —
    // `anchor-size()`, растяжка в клетке `position-area` — из реестра
    // ПРОШЛОГО кадра, до раскладки.
    merged.self_node = e.node_id;
    merged.anchor_seq = crate::layout::positioned::anchor::next_seq();
    merged.anchor_key = crate::layout::positioned::anchor::key_of(e);
    // Вариант `position-try-fallbacks`, выбранный на прошлом кадре, — в стиль
    // ДО размеров и раскладки (§fallback: «the element keeps those styles»).
    crate::layout::positioned::anchor::resolve::apply_chosen(&mut merged, vcx);
    crate::layout::positioned::anchor::resolve::resolve_sizes(&mut merged, inherited, vcx);
    // `dir="auto"` — сторона письма по ПЕРВОМУ СИЛЬНОМУ знаку содержимого.
    // Разбор двунаправленности выберет её сам при наборе, но выключка и
    // прижим текста читают `rtl` из стиля, и без этого шага блок с арабским
    // текстом прижимался влево.
    if e.attr("dir") == Some("auto") && e.style.rtl.is_none() {
        let mut text = String::new();
        gather_text(&e.children, &mut text);
        let strong = text.chars().find_map(|ch| {
            use unicode_bidi::BidiClass::*;
            match unicode_bidi::bidi_class(ch) {
                L => Some(false),
                R | AL => Some(true),
                _ => None,
            }
        });
        if let Some(rtl) = strong {
            merged.rtl = Some(rtl);
        }
    }
    // Предел ОРТОГОНАЛЬНОГО потока для строк вертикального письма внутри
    // (CSS Writing Modes §7.3). Искать его надо вверх по дереву, поэтому он
    // несётся вниз наследуемым полем. Ближе всего собственная определённая
    // высота элемента; за ней — высота ближайшего контейнера прокрутки с
    // наложенным на неё `max-height`; в самом конце — окно (его подставляет
    // потребитель в `paragraph`).
    orthogonal_limit(e, &mut merged, inherited, opts);
    pseudo_line_layers::install(e, &mut merged, vcx);
    // Единицы окна разрешаются здесь: размер окна знает только сборщик.
    if e.tag == "html-viewport" {
        // The synthetic viewport owns the full window; only author units shrink.
        merged.width = Some(crate::style::values::value::Len::Px(opts.viewport.0));
        if merged.height.is_some() {
            merged.height = Some(crate::style::values::value::Len::Px(opts.viewport.1));
        }
    }
    merged.resolve_viewport(opts.viewport, vcx);
    quirks::fill(e, &mut merged, inherited, opts);
    crate::animation::motion::apply_icb(&mut merged, opts.viewport, vcx);
    let gutters = crate::interactive::scroll_extent::classic::model(e.node_id, &merged);
    merged.scroll.used = gutters.used;
    merged.scroll.native = gutters.native;
    crate::layout::writing_mode::inline_definite::resolve_percentages(&mut merged, inherited);
    merged.inline_definite = crate::layout::writing_mode::inline_definite::own(
        &merged,
        inherited,
        e.tag == "html",
        opts.viewport.1,
    );
    if e.tag == "html-viewport" {
        merged.icb_scroll_insets = Some(gutters.used);
    }
    orthogonal_inline::resolve(&mut merged, inherited, opts.viewport, e.tag == "html");
    PAINT_VIEWPORT.with(|v| v.set(opts.viewport));
    let scroll_projection = crate::interactive::scroll_extent::projection(
        inherited.self_node,
        Some(&merged),
        opts.cx.sticky.as_ref(),
    );
    // Проецируемое поддерево строится в цепочке сдвига проекции (`ShiftLink`).
    let projected_opts = scroll_projection
        .as_ref()
        .map(|p| opts.inner(|cx| cx.sticky = Some(p.link().clone())));
    let opts = projected_opts.as_deref().unwrap_or(opts);
    // Элементы форм рисуются своим набором: без него поле ввода — пустой
    // прямоугольник, что выглядит поломкой разметки.
    if let Some(el) = crate::interactive::forms::element(e, &merged, opts) {
        return crate::interactive::scroll_extent::project(
            transformed(el, &merged, inherited),
            scroll_projection,
        );
    }
    // Рамка строится ОДИН раз до match: прежний `is_some() => unwrap()`
    // читал файл с диска и разбирал вложенный документ дважды за кадр.
    // Неразобранная рамка по-прежнему падает в общий рукав.
    let built_iframe = if e.tag == "iframe" {
        iframe(e, inherited, opts)
    } else if e.tag == "object" && object_is_document(e) {
        // `<object data="….html">` — тот же вложенный документ, что и рамка
        // (HTML §4.8.7): адрес переносится из `data` в `src`, размеры берутся
        // уже разрешёнными (`10em` в собственном стиле — ещё `Len::Em`).
        let mut copy = e.clone();
        let url = e.attr("data").unwrap_or_default().to_string();
        copy.attrs.push(("src".to_string(), url));
        copy.style.width = merged.width;
        copy.style.height = merged.height;
        iframe(&copy, inherited, opts)
    } else {
        None
    };
    crate::interactive::scroll_extent::project(
        dispatch_element(e, merged, inherited, opts, outer_row, built_iframe),
        scroll_projection,
    )
}
