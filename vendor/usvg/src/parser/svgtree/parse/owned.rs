//! Owned post-cascade resource nodes and collision-free instance import.
use super::*;

#[derive(Clone)]
pub(super) struct Snapshot {
    tag: Option<EId>,
    text: String,
    attrs: Vec<(AId, String, bool)>,
    children: Vec<Snapshot>,
}

impl Snapshot {
    pub(super) fn capture(node: super::super::SvgNode, depth: usize) -> Option<Self> {
        if depth > 256 {
            return None;
        }
        Some(Self {
            tag: node.tag_name(),
            text: if node.is_text() {
                node.text().to_owned()
            } else {
                String::new()
            },
            attrs: node
                .attributes()
                .iter()
                .map(|a| (a.name, a.value.to_string(), a.important))
                .collect(),
            children: node
                .children()
                .map(|n| Self::capture(n, depth + 1))
                .collect::<Option<_>>()?,
        })
    }

    // KaminIDE patch: select fragments from the single owned post-cascade document.
    pub(super) fn target(&self, fragment: &str) -> Option<&Self> {
        if fragment.is_empty() {
            return self.children.first();
        }
        self.find_id(fragment)
    }

    fn find_id(&self, id: &str) -> Option<&Self> {
        // Typed-tree links select the last occurrence of a duplicate id.
        self.children.iter().rev().find_map(|child| child.find_id(id)).or_else(|| {
            self.attrs.iter().any(|(name, value, _)| *name == AId::Id && value == id)
                .then_some(self)
        })
    }

    // KaminIDE patch: reserve owned strings, including prefixes added to references.
    // Counting every '#' is conservative for values that are not local references.
    pub(super) fn copy_bytes(&self, prefix_len: usize, ids: bool) -> u64 {
        let attrs = self.attrs.iter().filter(|(name, _, _)| ids || *name != AId::Id)
            .map(|(name, value, _)| {
                let prefixes = if *name == AId::Id { 1 } else { value.matches('#').count() };
                (value.len() + prefixes * prefix_len) as u64
            }).sum::<u64>();
        self.text.len() as u64 + attrs
            + self.children.iter().map(|n| n.copy_bytes(prefix_len, ids)).sum::<u64>()
    }

    pub(super) fn count(&self) -> usize {
        1 + self.children.iter().map(Self::count).sum::<usize>()
    }

    pub(super) fn append(&self, doc: &mut Document, parent: NodeId, prefix: &str, ids: bool) {
        if self.tag.is_none() && self.text.is_empty() {
            for child in &self.children {
                child.append(doc, parent, prefix, ids);
            }
            return;
        }
        let kind = if let Some(tag_name) = self.tag {
            let start = doc.attrs.len();
            for (name, value, important) in &self.attrs {
                if *name == AId::Id && !ids {
                    continue;
                }
                let value = if *name == AId::Id {
                    format!("{prefix}{value}")
                } else {
                    remap(*name, value, prefix)
                };
                doc.attrs.push(Attribute {
                    name: *name,
                    value: roxmltree::StringStorage::new_owned(&value),
                    important: *important,
                });
            }
            NodeKind::Element {
                tag_name,
                attributes: ShortRange::new(start as u32, doc.attrs.len() as u32),
            }
        } else {
            NodeKind::Text(self.text.clone())
        };
        let node = doc.append(parent, kind);
        if ids {
            if let Some(id) = doc.get(node).attribute::<&str>(AId::Id) {
                doc.links.insert(id.to_owned(), node);
            }
        }
        for child in &self.children {
            child.append(doc, node, prefix, ids);
        }
    }
}

fn remap(name: AId, value: &str, prefix: &str) -> String {
    if name == AId::Href && value.starts_with('#') {
        return format!("#{prefix}{}", &value[1..]);
    }
    let mut out = String::new();
    let mut rest = value;
    while let Some(at) = rest.find("url(") {
        out.push_str(&rest[..at + 4]);
        rest = &rest[at + 4..];
        let Some(end) = rest.find(')') else {
            break;
        };
        let iri = rest[..end].trim().trim_matches(['\'', '"']);
        if let Some(fragment) = iri.strip_prefix('#') {
            out.push_str(&format!("#{prefix}{fragment}"));
        } else {
            out.push_str(&rest[..end]);
        }
        out.push(')');
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}
