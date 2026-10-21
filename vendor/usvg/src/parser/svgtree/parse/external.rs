//! External use resolves a separate resource cascade before importing typed nodes.
use super::owned::Snapshot;
use super::*;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use url::Url;

mod budget;
use budget::Budget;

#[derive(Default)]
struct Session {
    active: HashSet<String>,
    cache: HashMap<String, Option<Arc<Resource>>>,
    budget: Budget,
}
struct Resource {
    file: String,
    definitions: Snapshot,
}

impl Document<'_> {
    pub(crate) fn import_external(&mut self, opt: &crate::Options, source_bytes: usize, source_nodes: usize) {
        let base = opt
            .resources_dir
            .as_ref()
            .and_then(|p| Url::from_directory_path(p).ok());
        let mut session = Session::default();
        if session.budget.bytes(source_bytes as u64) && session.budget.nodes(source_nodes + self.nodes.len()) {
            session.expand(self, base.as_ref(), 0);
        }
        super::super::cycles::fix_resource_cycles(self);
    }
}

impl Session {
    fn expand(&mut self, doc: &mut Document, base: Option<&Url>, depth: usize) {
        if depth >= 32 {
            return;
        }
        normalize_hrefs(doc, base);
        self.expand_from(doc, depth, 0);
    }

    fn expand_from(&mut self, doc: &mut Document, depth: usize, start: usize) {
        if depth >= 32 {
            return;
        }
        let uses: Vec<_> = (start..doc.nodes.len())
            .map(|i| doc.get(NodeId::from(i)))
            .filter(|n| n.tag_name() == Some(EId::Use))
            .filter_map(|n| n.attribute::<&str>(AId::Href).map(|h| (n.id, h.to_owned())))
            .collect();
        for (parent, href) in uses {
            if href.starts_with('#') || doc.get(parent).has_children() {
                continue;
            }
            let Some(uri) = resolve(&href, None) else {
                continue;
            };
            let fragment = uri.fragment().unwrap_or("").to_owned();
            let Some(resource) = self.load(uri, depth + 1) else {
                continue;
            };
            let Some(target) = resource.definitions.target(&fragment) else {
                continue;
            };
            // KaminIDE patch: instance cycles concern a target, not the whole file.
            // The file cache remains independent of the active fragment/depth.
            let key = format!("{}#{fragment}", resource.file);
            if self.active.contains(&key) {
                continue;
            }
            // KaminIDE patch: reserve both copies and the defs wrapper before importing.
            let prefix = format!("__kamin_resource_{}_", doc.nodes.len());
            let count = 1 + resource.definitions.count() + target.count();
            let bytes = resource.definitions.copy_bytes(prefix.len(), true)
                + target.copy_bytes(prefix.len(), false);
            if !self.budget.nodes(count) || !self.budget.bytes(bytes) {
                continue;
            }
            self.active.insert(key.clone());
            let start = doc.nodes.len();
            let empty = doc.attrs.len() as u32;
            let defs = doc.append(
                doc.root_element().id,
                NodeKind::Element {
                    tag_name: EId::Defs,
                    attributes: ShortRange::new(empty, empty),
                },
            );
            resource.definitions.append(doc, defs, &prefix, true);
            target.append(doc, parent, &prefix, false);
            self.expand_from(doc, depth + 1, start);
            self.active.remove(&key);
        }
    }

    fn load(&mut self, uri: Url, depth: usize) -> Option<Arc<Resource>> {
        let path = std::fs::canonicalize(uri.to_file_path().ok()?).ok()?;
        let canonical = Url::from_file_path(&path).ok()?;
        // KaminIDE patch: the parsed document belongs to the file, not a fragment.
        let key = canonical.to_string();
        if depth >= 32 {
            return None;
        }
        if let Some(resource) = self.cache.get(&key) {
            return resource.clone();
        }
        // Reserve cache metadata even for a file that will be rejected.
        if !self.budget.bytes(key.len() as u64) || !self.budget.nodes(1) {
            return None;
        }
        let result = (|| {
            let bytes = self.budget.read_resource(&path)?;
            let text = std::str::from_utf8(&bytes).ok()?;
            let xml = self.budget.parse_xml(text)?;
            let mut source = self.budget.parse_tree(&xml)?;
            // Keep the parsed file independent of the first instance's context.
            normalize_hrefs(&mut source, Some(&canonical));
            // Reserve the owned cache representation before capturing it.
            let bytes = source.attrs.iter().map(|a| a.value.len() as u64).sum::<u64>()
                + source.descendants().filter(|n| n.is_text()).map(|n| n.text().len() as u64).sum::<u64>();
            if !self.budget.nodes(source.nodes.len()) || !self.budget.bytes(bytes) {
                return None;
            }
            Some(Arc::new(Resource {
                file: key.clone(),
                definitions: Snapshot::capture(source.root(), 0)?,
            }))
        })();
        // Remember failures too: different fragments must not retry a rejected file.
        self.cache.insert(key, result.clone());
        result
    }
}

// KaminIDE patch: preserve resource bases in the cached file and every instance.
fn normalize_hrefs(doc: &mut Document, base: Option<&Url>) {
    for attr in &mut doc.attrs {
        if attr.name == AId::Href && !attr.value.starts_with('#') {
            if let Some(uri) = resolve(&attr.value, base) {
                let text = if uri.fragment().is_none() {
                    uri.to_file_path().ok()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or(uri.to_string())
                } else {
                    uri.to_string()
                };
                attr.value = roxmltree::StringStorage::new_owned(&text);
            }
        }
    }
}

fn resolve(href: &str, base: Option<&Url>) -> Option<Url> {
    // A file path containing a Windows drive is not a URL scheme.
    let path = std::path::Path::new(href.split('#').next()?);
    if path.is_absolute() {
        let mut uri = Url::from_file_path(path).ok()?;
        uri.set_fragment(href.split_once('#').map(|(_, f)| f));
        Some(uri)
    } else {
        Url::parse(href).ok().or_else(|| base?.join(href).ok())
    }
}

// KaminIDE patch: refuse oversized files before allocation and bound the read
// as well: metadata alone cannot protect against a file growing after open.
const RESOURCE_BYTE_LIMIT: u64 = 16 * 1024 * 1024;

fn read_bounded(reader: impl std::io::Read, limit: u64) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut bytes = Vec::new();
    reader.take(limit + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= limit).then_some(bytes)
}

#[cfg(test)]
mod tests;
