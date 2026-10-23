//! Слои `text-shadow` (css-text-decor-3 §4): копии текста под абзацем, первая тень сверху,
//! размытие через буфер группы. Плоский текст повторяет тот же параграф с цветом тени, чтобы
//! baseline, интервалы и декорации совпали; строчные коробки с разными наборами теней дают
//! отдельные группы с масками, атомные коробки рисуют тени своим путём.

mod groups;
mod masks;
use crate::hooks::IntoHooked;
use groups::{ShadowStyle, collect_groups};
use masks::masked_layer;

use crate::dom::Node;
use crate::dom::text_content::gather_text;
use crate::layout::options::RenderOpts;
use crate::style::computed::Shadow;
use crate::text::shadow_text::normalize_for_shadow;
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, div, px};

fn shadow_layer(
    text: &str,
    shadow: &Shadow,
    nodes: &[Node],
    style: &crate::style::computed::BoxStyle,
    opts: &RenderOpts,
    same_paragraph: bool,
) -> AnyElement {
    let vcx = opts.cx.vcx();
    let copy = if same_paragraph {
        let mut mask = style.clone();
        mask.text_shadow_ink = Some(shadow.used_color(style.current_color()));
        mask.text_shadow = None;
        mask.text_shadow_rest.clear();
        mask.text_shadow_raw = None;
        // CSS Text Decoration 3 §4: a shadow masks both glyphs and their
        // decorations. Reusing the paragraph keeps baseline, spacing and
        // decoration geometry identical; its color is the shadow's color.
        opts.cx.build.paragraph(nodes, &mask, opts)
    } else {
        div()
            .text_color(shadow.used_color(style.current_color()).to_hsla())
            .child(SharedString::from(text.to_string()))
            .into_any_element()
    };
    let copy = if shadow.blur > 0.5 {
        let mut group = crate::paint::effects::grouped_element::Grouped::new(copy, vcx);
        group.blur = shadow.blur * 0.5;
        group.hooked()
    } else {
        copy
    };
    // The offset belongs outside the blur buffer so its child has bounds.
    let mut placed = div().absolute().left(px(shadow.x)).top(px(shadow.y));
    if same_paragraph {
        placed = placed.w_full();
    }
    placed.child(copy).into_any_element()
}

pub(crate) fn with_text_shadow(
    el: AnyElement,
    style: &crate::style::computed::BoxStyle,
    nodes: &[Node],
    opts: &RenderOpts,
) -> AnyElement {
    let context = ShadowStyle::new(style);
    let block = context.resolved();
    let all_text = nodes.iter().all(|n| matches!(n, Node::Text(_)));
    let flat = !style.wm().vertical()
        && !style.wm().turned()
        && style.first_line.is_none()
        && style.first_letter.is_none()
        && style.text_emphasis.is_none();
    // Тени строчных коробок (css-text-decor-3 §4: `text-shadow` действует на
    // текст КАЖДОЙ коробки, не только блока): группы по набору теней, в
    // порядке текста. Прежде тень `<span>` или `::marker` не рисовалась
    // вовсе, а тень блока со строчными детьми шла голым текстом мимо
    // раскладки абзаца.
    let mut groups: Vec<Vec<Shadow>> = Vec::new();
    if flat && !all_text {
        collect_groups(nodes, &context, &mut groups, opts.cx.vcx());
    }
    if !groups.is_empty() {
        let mut layers = Vec::new();
        for group in &groups {
            // Первая тень — сверху, все — под текстом (§4).
            for shadow in group.iter().rev() {
                layers.push(masked_layer(group, shadow, &context, nodes, style, opts));
            }
        }
        return div()
            .relative()
            .children(layers)
            .child(el)
            .into_any_element();
    }
    let Some((shadow, rest)) = block.split_first() else {
        return el;
    };
    let mut plain = String::new();
    gather_text(nodes, &mut plain);
    let plain = crate::text::inline::transform_case(&normalize_for_shadow(&plain), style);
    if plain.trim().is_empty() {
        return el;
    }
    // Mixed inline styles and rotated paragraphs retain their existing route.
    let same_paragraph = all_text && flat;
    // CSS Text Decoration 3 §4: first shadow is on top, all below the text.
    let layers = rest
        .iter()
        .rev()
        .chain(std::iter::once(shadow))
        .map(|s| shadow_layer(plain.trim(), s, nodes, style, opts, same_paragraph));
    div()
        .relative()
        .children(layers)
        .child(el)
        .into_any_element()
}

#[cfg(test)]
fn shadow_list(c: &crate::style::computed::Computed) -> Vec<Shadow> {
    ShadowStyle::new(c).resolved()
}

/// Атомная или блочная коробка внутри абзаца: её содержимое в слой тени
/// строк не идёт (рисуется своим путём).
fn atomic(e: &crate::dom::Element, vcx: crate::document::state::ValueCx<'_>) -> bool {
    // Кусок со своей коробкой абзац ставит атомом со СВОИМ абзацем внутри,
    // и тень тот рисует сам (`render::has_own_box`).
    let font = match e.style.computed().font_size {
        Some(crate::style::values::value::Len::Px(v)) => v,
        _ => 16.0,
    };
    !e.inline
        || crate::layout::positioned::predicates::has_own_box(&e.style.computed(), font, vcx)
        || matches!(
            e.tag.as_str(),
            "img"
                | "svg"
                | "canvas"
                | "video"
                | "embed"
                | "object"
                | "iframe"
                | "input"
                | "button"
                | "select"
                | "textarea"
                | "ruby"
        )
}

#[cfg(test)]
mod tests;
