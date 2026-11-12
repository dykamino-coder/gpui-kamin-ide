//! Использованный стиль коробки элемента перед сборкой.
//!
//! Слияние своего стиля с наследованием и всё, что знает только сборщик:
//! ключи реестров кадра, `dir="auto"`, предел ортогонального потока, единицы
//! окна, причуды, полосы прокрутки и определённость строчного размера.
//! Один расчёт для общего пути элемента и для быстрых путей сборки.

use super::{orthogonal_limit, quirks};
use crate::dom::Element;
use crate::dom::text_content::gather_text;
use crate::layout::box_tree::pseudo_line_layers;
use crate::layout::options::RenderOpts;
use crate::layout::writing_mode::orthogonal_inline;
use crate::style::cascade::materialize::box_style;
use crate::style::computed::BoxStyle;

pub(crate) fn merged_style(e: &Element, inherited: &BoxStyle, opts: &RenderOpts) -> BoxStyle {
    let vcx = opts.cx.vcx();
    let mut merged = box_style(inherited, &e.style, vcx);
    crate::layout::block::inline_splits::restore_containing_block(e, &mut merged);
    // Ключи реестров и размеры из опубликованной геометрии текущего кадра.
    merged.self_node = e.node_id;
    merged.anchor_seq = opts.cx.anchor_seq_for(e);
    merged.anchor_key = crate::layout::positioned::anchor::key_of(e);
    crate::layout::positioned::anchor::resolve::prepare(&mut merged, inherited, &opts.cx);
    // `dir="auto"` — сторона письма по ПЕРВОМУ СИЛЬНОМУ знаку содержимого.
    // Разбор двунаправленности выберет её сам при наборе, но выключка и
    // прижим текста читают `rtl` из стиля, и без этого шага блок с арабским
    // текстом прижимался влево.
    if e.attr("dir") == Some("auto") && e.style.cascaded().inh.writing.rtl.is_none() {
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
            merged.inh.writing.rtl = Some(rtl);
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
        merged.reset_box_layout.width = Some(crate::style::values::value::Len::Px(opts.viewport.0));
        if merged.reset_box_layout.height.is_some() {
            merged.reset_box_layout.height =
                Some(crate::style::values::value::Len::Px(opts.viewport.1));
        }
    }
    merged.resolve_viewport(opts.viewport, vcx);
    quirks::fill(e, &mut merged, inherited, opts);
    crate::animation::motion::apply_icb(&mut merged, opts.viewport, vcx);
    let gutters = crate::interactive::scroll_extent::classic::model(
        opts.cx.scroll.frame.as_ref(),
        e.node_id,
        &merged,
    );
    // Equal values are not written, so the box-layout group stays shared with
    // the computed style (Blink `ComputedStyle` setters compare first).
    if merged.reset_box_layout.scroll.used != gutters.used
        || merged.reset_box_layout.scroll.native != gutters.native
    {
        merged.reset_box_layout.scroll.used = gutters.used;
        merged.reset_box_layout.scroll.native = gutters.native;
    }
    crate::layout::writing_mode::inline_definite::resolve_percentages(&mut merged, inherited);
    merged.inline_definite = crate::layout::writing_mode::inline_definite::own(
        &merged,
        inherited,
        e.tag == "html",
        opts.viewport.1,
    );
    if e.tag == "html-viewport" {
        merged.reset_positioned.icb_scroll_insets = Some(gutters.used);
    }
    orthogonal_inline::resolve(&mut merged, inherited, opts.viewport, e.tag == "html");
    merged
}
