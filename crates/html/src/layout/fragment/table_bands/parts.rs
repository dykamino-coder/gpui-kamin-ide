//! Порядок секций таблицы и параметры ячеек для меры и поиска переносов.

use crate::dom::{Element, Node};
use crate::layout::fragment::ShapeCx;
use crate::render::is_blank;
use crate::style::computed::Display;
use crate::style::values::value::Len;

/// `cellspacing`/`cellpadding` уступают авторскому стилю, но заменяют
/// каскадное умолчание тега `<table>` (2px); у `display: table` его нет.
pub(crate) fn cell_settings(c: &Element, cx: ShapeCx) -> Option<(f32, ShapeCx)> {
    let px_of = |l: &Option<Len>| match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    };
    let attr_px = |name: &str| {
        c.attr(name)
            .and_then(|v| v.trim().trim_end_matches("px").parse::<f32>().ok())
    };
    let ua_default = matches!(
        c.style.border_spacing,
        Some((Some(Len::Px(2.0)), Some(Len::Px(2.0))))
    );
    let spacing = match (attr_px("cellspacing"), ua_default, &c.style.border_spacing) {
        (Some(v), true, _) | (Some(v), _, None) => v,
        (_, _, Some((_, y))) => px_of(y)?,
        _ => 0.0,
    };
    let cell_cx = ShapeCx {
        cell_pad: attr_px("cellpadding"),
        ..cx
    };
    Some((spacing, cell_cx))
}

pub(crate) fn is_row(e: &Element) -> bool {
    e.tag == "tr" || e.style.display == Some(Display::TableRow)
}

fn is_group(e: &Element) -> bool {
    matches!(e.tag.as_str(), "thead" | "tbody" | "tfoot")
        || e.style.display == Some(Display::TableRowGroup)
        || e.style.row_group_kind.is_some()
}

/// Подписи обрабатывает вызывающий; остальные части сортируются стабильно.
pub(crate) fn ordered_parts<'a>(
    fixed: &'a [Node],
    mut skip_caption: impl FnMut(&'a Element) -> bool,
) -> Option<Vec<(u8, &'a Element)>> {
    let mut parts: Vec<(u8, &Element)> = Vec::new();
    let (mut head, mut foot) = (false, false);
    for n in fixed.iter().filter(|n| !is_blank(n)) {
        let Node::Element(e) = n else { return None };
        if skip_caption(e) {
            continue;
        }
        let role = match e.tag.as_str() {
            "thead" => Some(0u8),
            "tbody" => Some(1),
            "tfoot" => Some(2),
            _ => e.style.row_group_kind,
        };
        let kind = match role {
            Some(0) if !head => {
                head = true;
                0
            }
            Some(2) if !foot => {
                foot = true;
                2
            }
            _ if is_row(e) || is_group(e) => 1,
            _ => return None,
        };
        parts.push((kind, e));
    }
    parts.sort_by_key(|p| p.0);
    Some(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn element(tag: &str) -> Element {
        Element {
            tag: tag.into(),
            style: Default::default(),
            children: Vec::new(),
            attrs: Vec::new(),
            node_id: 0,
            inline: false,
            hover: None,
            first_letter: None,
            first_line: None,
            anim: None,
            list_item: None,
        }
    }

    #[test]
    fn only_first_header_and_footer_move_and_captions_are_separate() {
        let tags = ["tfoot", "tbody", "thead", "thead", "tfoot", "tr", "caption"];
        let nodes: Vec<_> = tags.iter().map(|tag| Node::Element(element(tag))).collect();
        assert!(ordered_parts(&nodes, |_| false).is_none());
        let mut captions = Vec::new();
        let parts = ordered_parts(&nodes, |e| {
            if e.tag == "caption" {
                captions.push(e);
                true
            } else {
                false
            }
        })
        .unwrap();
        assert_eq!(
            parts
                .iter()
                .map(|(_, e)| e.tag.as_str())
                .collect::<Vec<_>>(),
            ["thead", "tbody", "thead", "tfoot", "tr", "tfoot"]
        );
        assert!(std::ptr::eq(
            parts[0].1,
            match &nodes[2] {
                Node::Element(e) => e,
                _ => unreachable!(),
            }
        ));
        assert_eq!(captions.len(), 1);
    }

    #[test]
    fn author_spacing_overrides_attribute_but_ua_default_does_not() {
        let mut table = element("table");
        table.attrs = vec![
            ("cellspacing".into(), " 7px ".into()),
            ("cellpadding".into(), "3".into()),
        ];
        table.style.border_spacing = Some((Some(Len::Px(2.0)), Some(Len::Px(2.0))));
        let (spacing, cx) = cell_settings(&table, ShapeCx::COLUMNS).unwrap();
        assert_eq!(spacing, 7.0);
        assert_eq!(cx.cell_pad, Some(3.0));
        table.style.border_spacing = Some((Some(Len::Px(5.0)), Some(Len::Px(9.0))));
        assert_eq!(cell_settings(&table, ShapeCx::COLUMNS).unwrap().0, 9.0);
        table.style.border_spacing = None;
        assert_eq!(cell_settings(&table, ShapeCx::COLUMNS).unwrap().0, 7.0);
    }
}
