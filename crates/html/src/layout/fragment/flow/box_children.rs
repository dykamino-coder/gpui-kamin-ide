//! Дети коробки по CSS Display 3 §2.5: contents прозрачен, none не создаёт поддерева.
//! DOM для отрисовки сохраняется: наследование через contents остаётся у renderer.

use crate::dom::Node;
use crate::style::computed::Display;

pub(crate) fn children(nodes: &[Node]) -> impl Iterator<Item = &Node> {
    let mut stack = vec![nodes.iter()];
    std::iter::from_fn(move || {
        loop {
            match stack.last_mut()?.next() {
                Some(Node::Element(e)) if e.style.display == Some(Display::None) => {}
                Some(Node::Element(e)) if e.style.display == Some(Display::Contents) => {
                    stack.push(e.children.iter());
                }
                Some(n) => return Some(n),
                None => {
                    stack.pop();
                }
            }
        }
    })
}

/// Прямые дети фрагментаинера становятся самостоятельными копиями. При
/// снятии contents сохраняется CSS-родитель наследования (Cascade 4 §7),
/// а голый текст получает анонимную строчную коробку, как при table fixup.
pub(crate) fn projected(
    nodes: &[Node],
    parent: &crate::style::computed::Computed,
    vcx: crate::document::state::ValueCx<'_>,
) -> Vec<Node> {
    use crate::style::cascade::inherit::inherit_unpainted;
    use crate::style::computed::Computed;
    let mut out = Vec::new();
    for n in nodes {
        match n {
            Node::Element(e) if e.style.display == Some(Display::None) => {}
            Node::Element(e) if e.style.display == Some(Display::Contents) => {
                let merged = inherit_unpainted(parent, &e.style, vcx);
                let promoted: Vec<_> = e
                    .children
                    .iter()
                    .map(|n| match n {
                        Node::Element(k) => {
                            let mut k = k.clone();
                            k.style =
                                resolved_copy(inherit_unpainted(&merged, &k.style, vcx)).into();
                            Node::Element(k)
                        }
                        Node::Text(t) => {
                            let mut span = crate::layout::table::anon::anon_element(
                                "span",
                                vec![Node::Text(t.clone())],
                            );
                            span.inline = true;
                            span.style = resolved_copy(inherit_unpainted(
                                &merged,
                                &Computed::default(),
                                vcx,
                            ))
                            .into();
                            Node::Element(span)
                        }
                    })
                    .collect();
                out.extend(projected(&promoted, parent, vcx));
            }
            other => out.push(other.clone()),
        }
    }
    out
}

// CSS Cascade 4 §7: после смены box-родителя результат наследования остаётся
// вычисленным значением. Повторная сборка копии пересчитывает containing-block
// metadata, но не должна заново исполнять директивы от прежнего CSS-родителя.
fn resolved_copy(mut c: crate::style::computed::Computed) -> crate::style::computed::Computed {
    c.explicit_inherit.clear();
    c.inherit_bits = 0;
    c.grid_areas_inherit = false;
    c.border_inherit = false;
    c.border_inherit_w = [false; 4];
    c.border_inherit_s = [false; 4];
    c.border_inherit_c = [false; 4];
    c.padding_inherit = false;
    c.margin_inherit = [false; 4];
    c.padding_inherit_side = [false; 4];
    c.shadow_inherit = false;
    c.background_inherit = false;
    c.background_all_inherit = false;
    c.width_inherit = false;
    c.height_inherit = false;
    c.minmax_inherit = [false; 4];
    c.inset_inherit = [false; 4];
    c.text_overflow_inherit = false;
    c.align_self_inherit = false;
    c.align_inherit = 0;
    c.bidi_inherit = false;
    c.clear_inherit = false;
    c.content_inherit = false;
    c.text_align_match_parent = 0;
    c.font_size_step = 0;
    c.font_weight_step = 0;
    c.scroll.inherit_gutter = false;
    c.scroll.inherit_width = false;
    c.snap.inherit = 0;
    c
}

#[cfg(test)]
mod tests;
