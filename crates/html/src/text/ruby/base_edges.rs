//! Крайние знаки базового уровня руби для переноса строк. Руби — строчная коробка, а не
//! атомарная (css-ruby-1 §2.1): точки переноса у её краёв даёт содержимое баз, как у соседнего
//! текста (css-text-3 §5.1, UAX #14), а не знак-заместитель объекта. Аннотации (`rt`, `rtc`)
//! и `rp` на базовом уровне не стоят; краевые пробелы контейнера отбрасываются (§2.4).

use super::ruby_role;
use crate::dom::{Element, Node};
use crate::style::computed::RubyRole;

/// First and last base-level characters of the ruby container `e`; `None`
/// for an empty base level. Atoms inside the bases read as U+FFFC.
pub(crate) fn base_edges(e: &Element) -> (Option<char>, Option<char>) {
    let mut edges = (None, None);
    walk(&e.children, &mut edges);
    edges
}

fn walk(nodes: &[Node], edges: &mut (Option<char>, Option<char>)) {
    for n in nodes {
        match n {
            Node::Text(t) => t
                .chars()
                .filter(|c| !c.is_whitespace())
                .for_each(|c| seen(edges, c)),
            Node::Element(k)
                if k.tag == "rp"
                    || matches!(ruby_role(k), Some(RubyRole::Text | RubyRole::TextContainer)) => {}
            Node::Element(k)
                if !crate::layout::box_kind::replaced_tag(k)
                    && (k.inline || ruby_role(k).is_some()) =>
            {
                walk(&k.children, edges);
            }
            Node::Element(_) => seen(edges, '\u{fffc}'),
        }
    }
}

fn seen(edges: &mut (Option<char>, Option<char>), c: char) {
    edges.0.get_or_insert(c);
    edges.1 = Some(c);
}

#[cfg(test)]
mod tests {
    use super::base_edges;
    use crate::document::parsed::parse;
    use crate::dom::{Element, Node};

    fn ruby(html: &str) -> (Option<char>, Option<char>) {
        fn find(nodes: &[Node]) -> Option<&Element> {
            nodes.iter().find_map(|n| match n {
                Node::Element(e) if e.tag == "ruby" => Some(e),
                Node::Element(e) => find(&e.children),
                Node::Text(_) => None,
            })
        }
        base_edges(find(&parse(html, "").nodes).expect("ruby"))
    }

    #[test]
    fn edges_skip_annotations_parentheses_and_white_space() {
        assert_eq!(
            ruby("<ruby> <rb>漢</rb><rp>(</rp><rt>kan</rt><rp>)</rp> 字 <rt>ji</rt></ruby>"),
            (Some('漢'), Some('字'))
        );
        assert_eq!(ruby("<ruby>\n <rt>x</rt>\n</ruby>"), (None, None));
    }
}
