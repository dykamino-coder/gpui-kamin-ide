//! Паспорта исходного DOM и однократная перепись структурных позиций.

use super::*;
/// Паспорт элемента для сопоставления селекторов.
pub fn ancestor_of(child: &Handle, spot: Spot, tree: &TreeIndex) -> Option<Ancestor> {
    let NodeData::Element { name, attrs, .. } = &child.data else {
        return None;
    };
    let attrs = attrs.borrow();
    let find = |key: &str| {
        attrs
            .iter()
            .find(|a| &*a.name.local == key)
            .map(|a| a.value.to_string())
    };
    Some(Ancestor {
        key: crate::dom::tree_index::NodeKey::of(child),
        matching: tree.matching.clone(),
        ancestor_bloom: None,
        counter_style: Default::default(),
        counter_context: Default::default(),
        content_style: None,
        content_none: None,
        tag: local_name(&name.local),
        html_attrs: content::html_attributes(&name.ns, tree.xhtml),
        id: find("id"),
        classes: find("class")
            .map(|v| v.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default(),
        attrs: attrs
            .iter()
            .map(|a| (a.name.local.to_string(), a.value.to_string()))
            .collect(),
        spot,
        href: find("href"),
        visited: link_visited(find("href").as_deref()),
        dir: find("dir").and_then(|v| match v.to_ascii_lowercase().as_str() {
            "rtl" => Some(true),
            "ltr" => Some(false),
            _ => None,
        }),
        has_marks: tree.has_marks_of(child),
        shadow_has_marks: vec![],
        featureless: None,
        slot: tree.slot_of(child),
        peers: None,
        target_group: None,
        target_current: false,
        empty: is_empty_node(child),
        foreign: &*name.ns != "http://www.w3.org/1999/xhtml",
        lang: None,
    })
}

/// Посещена ли ссылка с адресом `href`: свою страницу браузер кладёт в
/// историю по определению, поэтому пустой адрес и якорь на себя посещены;
/// остальное для нас не посещено.
pub fn link_visited(href: Option<&str>) -> bool {
    href.is_some_and(|h| h.is_empty() || h.starts_with('#'))
}

/// Перепись детей уровня: места и паспорта всех элементов.
pub fn census_of(children: &[Handle], tree: &TreeIndex) -> (Vec<Spot>, Vec<Ancestor>) {
    // Один подсчёт итогов по прежнему ключу: точное локальное имя, без namespace.
    let mut counts: HashMap<&str, (usize, usize)> = HashMap::new();
    let mut total = 0;
    for child in children {
        if let NodeData::Element { name, .. } = &child.data {
            counts.entry(&name.local).or_default().0 += 1;
            total += 1;
        }
    }
    let mut seen = 0usize;
    let mut spots: Vec<Spot> = Vec::with_capacity(children.len());
    let mut all: Vec<Ancestor> = Vec::with_capacity(total);
    for child in children {
        let spot = match &child.data {
            NodeData::Element { name, .. } => {
                seen += 1;
                let (of_type_total, of_type) = counts.entry(&name.local).or_default();
                *of_type += 1;
                Spot {
                    index: seen,
                    total,
                    of_type: *of_type,
                    of_type_total: *of_type_total,
                }
            }
            _ => Spot::default(),
        };
        spots.push(spot);
        if let Some(a) = ancestor_of(child, spot, tree) {
            all.push(a);
        }
    }
    (spots, all)
}

/// `:empty` (selectors-4 §14.2): ни детей-элементов, ни текста; комментарии и
/// инструкции обработки не считаются, а пробельный текст — считается, как в
/// браузерах (`image-light-dark`: `<div></div>` пуст, `<div><div>` — нет).
pub fn is_empty_node(handle: &Handle) -> bool {
    handle.children.borrow().iter().all(|c| match &c.data {
        NodeData::Element { .. } => false,
        NodeData::Text { contents } => contents.borrow().is_empty(),
        _ => true,
    })
}
