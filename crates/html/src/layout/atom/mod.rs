//! Строчные атомы — не-текстовые инлайн-элементы, которые нельзя встроить в поток текста
//! (`inline-block`, `inline-flex`, `inline-grid`, `inline-table`, замещённые, формы).
//! Вход: элемент DOM со стилем, унаследованный стиль и `RenderOpts`; выход: готовый
//! элемент GPUI для ряда строки. `atom_element` выбирает путь: прокручиваемый контейнер
//! (CSS Overflow 3 §3), повёрнутый атом вертикального письма (`rotated_atom`), голый атом
//! (`raw`) и обёртка размера по содержимому (`width: min-content` и родня, css-sizing-3
//! §5.1). Подмодули: собственная коробка, доли (`pct`), позиционирование, замещённые.

use crate::dom::Element;
use crate::layout::box_kind::replaced_tag;
use crate::layout::intrinsic_wrap::content_sized_wraps;
use crate::layout::options::RenderOpts;
use crate::layout::positioned::static_position::at_static_position;
use crate::layout::writing_mode::rotated_atom;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::AnyElement;

mod own_box;
mod pct;
pub(crate) mod positioned;
mod replaced;
pub(super) use pct::pct_resolved_against_block;
pub(crate) use pct::pct_resolved_for_wrapper;
mod raw;
use raw::atom_element_raw;

/// Не-текстовые инлайн-элементы, которые в поток встроить нельзя.
/// Строчный атом с размером по ключевому слову (`width: min-content` и
/// родня): дорожка по содержимому ставится той же обёрткой, что у блочного
/// пути (`content_sized`). Без неё атом шёл в ряд строки голым, гибкий ряд
/// брал его основу по max-content, и `inline-grid`/`inline grid-lanes` с
/// `width: min-content` раскладывался по max-content: доли `1fr 2fr 1fr 1fr`
/// при базах по 2ch раздавались, вторая дорожка выходила 4ch
/// (`grid-lanes-intrinsic-sizing-cols-002-fr`; css-sizing-3 §5.1).
pub(crate) fn atom_element(
    e: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
) -> Option<AnyElement> {
    let vcx = opts.cx.vcx();
    // Atomic inline containers own a scrollport just like block containers
    // (CSS Overflow 3 §3). The raw atom path otherwise only clips their content.
    if !e.style.scroller
        && e.style.snap.axis != 0
        && !replaced_tag(e)
        && matches!(
            e.style.display,
            Some(
                crate::style::computed::Display::InlineBlock
                    | crate::style::computed::Display::InlineFlex
                    | crate::style::computed::Display::InlineGrid
                    | crate::style::computed::Display::InlineTable
            )
        )
        && e.style.inline_display != Some(true)
        && let Some(scroll) = crate::interactive::scroll_box::scrollable(e, inherited, opts)
    {
        return Some(scroll);
    }
    if let Some(physical) = rotated_atom::physical(e, inherited, opts) {
        return Some(physical);
    }
    let resolved = rotated_atom::resolved(e, inherited);
    let e = resolved.as_ref().unwrap_or(e);
    let el = atom_element_raw(e, inherited, opts)?;
    // CSS Sizing 3 §5.1: intrinsic keywords in the block axis behave as auto.
    let keyword = |l: Option<Len>| {
        matches!(
            l,
            Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent)
        )
    };
    // Повёрнутый абзац вертикального письма (`rotated_line`) набирается
    // горизонтально, но ось строки там вертикальна.
    let inline_axis_only = !inherited.wm().vertical()
        && !inherited.wm().turned()
        && !e.style.wm().vertical()
        && keyword(e.style.width)
        && !keyword(e.style.height);
    let wraps = inline_axis_only
        && content_sized_wraps(e)
        && !replaced_tag(e)
        && !at_static_position(&e.style)
        && !matches!(e.tag.as_str(), "input" | "select" | "textarea" | "button");
    Some(if wraps {
        opts.cx.build.content_sized(
            el,
            &e.style,
            &inherit(inherited, &e.style, vcx),
            (None, None),
            &opts.cx,
        )
    } else {
        el
    })
}
