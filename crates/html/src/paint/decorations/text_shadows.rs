//! Paint text shadows with the same paragraph geometry and decoration mask.

use crate::dom::Node;
use crate::render::{RenderOpts, gather_text, normalize_for_shadow, paragraph};
use crate::style::computed::{Computed, Shadow};
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, div, px};

fn shadow_layer(
    text: &str,
    shadow: &Shadow,
    nodes: &[Node],
    style: &Computed,
    opts: &RenderOpts,
    same_paragraph: bool,
) -> AnyElement {
    let copy = if same_paragraph {
        let mut mask = style.clone();
        mask.color = Some(shadow.color);
        mask.td_color = Some(shadow.color);
        for decor in &mut mask.decors {
            decor.color = shadow.color;
        }
        mask.text_shadow = None;
        mask.text_shadow_rest.clear();
        mask.text_shadow_raw = None;
        // CSS Text Decoration 3 §4: a shadow masks both glyphs and their
        // decorations. Reusing the paragraph keeps baseline, spacing and
        // decoration geometry identical; its color is the shadow's color.
        paragraph(nodes, &mask, opts)
    } else {
        div()
            .text_color(shadow.color.to_hsla())
            .child(SharedString::from(text.to_string()))
            .into_any_element()
    };
    let copy = if shadow.blur > 0.5 {
        let mut group = crate::paint::effects::grouped_element::Grouped::new(copy);
        group.blur = shadow.blur * 0.5;
        group.into_any_element()
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
    style: &Computed,
    nodes: &[Node],
    opts: &RenderOpts,
) -> AnyElement {
    let block = shadow_list(style);
    let all_text = nodes.iter().all(|n| matches!(n, Node::Text(_)));
    let flat = style.vertical != Some(true)
        && style.rotated_line != Some(true)
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
        collect_groups(nodes, &block, &mut groups);
    }
    if !groups.is_empty() {
        let mut layers = Vec::new();
        for group in &groups {
            // Первая тень — сверху, все — под текстом (§4).
            for shadow in group.iter().rev() {
                layers.push(masked_layer(group, shadow, &block, nodes, style, opts));
            }
        }
        return div()
            .relative()
            .children(layers)
            .child(el)
            .into_any_element();
    }
    let Some(shadow) = style.text_shadow else {
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
    let layers = style
        .text_shadow_rest
        .iter()
        .rev()
        .chain(std::iter::once(&shadow))
        .map(|s| shadow_layer(plain.trim(), s, nodes, style, opts, same_paragraph));
    div()
        .relative()
        .children(layers)
        .child(el)
        .into_any_element()
}

/// Список теней стиля: первая и хвост.
fn shadow_list(c: &Computed) -> Vec<Shadow> {
    c.text_shadow
        .into_iter()
        .chain(c.text_shadow_rest.iter().copied())
        .collect()
}

/// Действующие тени элемента: своя запись (в том числе `none`) сильнее
/// унаследованной — `text-shadow` наследуется списком целиком.
fn effective(e: &crate::dom::Element, parent: &[Shadow]) -> Vec<Shadow> {
    if e.style.text_shadow_none {
        Vec::new()
    } else if e.style.text_shadow.is_some() {
        shadow_list(&e.style)
    } else {
        parent.to_vec()
    }
}

/// Атомная или блочная коробка внутри абзаца: её содержимое в слой тени
/// строк не идёт (рисуется своим путём).
fn atomic(e: &crate::dom::Element) -> bool {
    // Кусок со своей коробкой абзац ставит атомом со СВОИМ абзацем внутри,
    // и тень тот рисует сам (`render::has_own_box`).
    let font = match e.style.font_size {
        Some(crate::style::values::value::Len::Px(v)) => v,
        _ => 16.0,
    };
    !e.inline
        || crate::layout::positioned::predicates::has_own_box(&e.style, font)
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

fn collect_groups(nodes: &[Node], current: &[Shadow], out: &mut Vec<Vec<Shadow>>) {
    for n in nodes {
        match n {
            Node::Text(t) => {
                if !current.is_empty()
                    && !t.trim().is_empty()
                    && !out.iter().any(|g| g.as_slice() == current)
                {
                    out.push(current.to_vec());
                }
            }
            Node::Element(e) => {
                if atomic(e) {
                    continue;
                }
                let list = effective(e, current);
                collect_groups(&e.children, &list, out);
            }
        }
    }
}

fn transparent() -> crate::style::values::value::Color {
    crate::style::values::value::Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    }
}

/// Слой одной тени строчных коробок: тот же абзац (геометрия строк та же),
/// где текст коробок этой группы окрашен в цвет тени, а всё прочее —
/// прозрачно: чужой текст, фоны, рамки, контуры и атомы.
fn masked_layer(
    group: &[Shadow],
    shadow: &Shadow,
    block: &[Shadow],
    nodes: &[Node],
    style: &Computed,
    opts: &RenderOpts,
) -> AnyElement {
    let on = block == group;
    let ink = if on { shadow.color } else { transparent() };
    let mut mask = style.clone();
    mask.color = Some(ink);
    mask.td_color = Some(ink);
    mask.emphasis_color = Some(ink);
    for decor in &mut mask.decors {
        decor.color = ink;
    }
    mask.text_shadow = None;
    mask.text_shadow_rest.clear();
    mask.text_shadow_raw = None;
    let masked = mask_nodes(nodes, block, &style.decors, &mask.decors, group, shadow);
    let copy = paragraph(&masked, &mask, opts);
    let copy = if shadow.blur > 0.5 {
        let mut g = crate::paint::effects::grouped_element::Grouped::new(copy);
        g.blur = shadow.blur * 0.5;
        g.into_any_element()
    } else {
        copy
    };
    div()
        .absolute()
        .left(px(shadow.x))
        .top(px(shadow.y))
        .w_full()
        .child(copy)
        .into_any_element()
}

fn mask_nodes(
    nodes: &[Node],
    parent: &[Shadow],
    parent_decors: &[crate::style::computed::Decor],
    parent_masked: &[crate::style::computed::Decor],
    group: &[Shadow],
    shadow: &Shadow,
) -> Vec<Node> {
    nodes
        .iter()
        .map(|n| match n {
            Node::Text(_) => n.clone(),
            Node::Element(e) => {
                let mut e = e.clone();
                if atomic(&e) {
                    e.style.hidden = Some(true);
                    return Node::Element(e);
                }
                let list = effective(&e, parent);
                let ink = if list.as_slice() == group {
                    shadow.color
                } else {
                    transparent()
                };
                let own_decors = e.style.decors.clone();
                let s = &mut e.style;
                s.color = Some(ink);
                s.td_color = Some(ink);
                s.emphasis_color = Some(ink);
                // Украшения предков красит тень предка: ведущие записи,
                // совпадающие с родительскими, берут маску родителя.
                for (i, decor) in s.decors.iter_mut().enumerate() {
                    decor.color = if i < parent_decors.len() && parent_decors[i] == own_decors[i] {
                        parent_masked.get(i).map_or(ink, |d| d.color)
                    } else {
                        ink
                    };
                }
                s.background = None;
                s.bg_image = None;
                s.shadows.clear();
                s.outline = None;
                s.border_color = Some(transparent());
                s.border_colors = [Some(transparent()); 4];
                s.text_shadow = None;
                s.text_shadow_rest.clear();
                s.text_shadow_raw = None;
                let masked_decors = e.style.decors.clone();
                let (own, masked) = if own_decors.is_empty() {
                    (parent_decors.to_vec(), parent_masked.to_vec())
                } else {
                    (own_decors, masked_decors)
                };
                e.children = mask_nodes(&e.children, &list, &own, &masked, group, shadow);
                Node::Element(e)
            }
        })
        .collect()
}
