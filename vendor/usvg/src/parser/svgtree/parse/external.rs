//! External use resolves a separate resource cascade before importing typed nodes.
use super::owned::Snapshot;
use super::*;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use url::Url;

#[derive(Default)]
struct Session {
    active: HashSet<String>,
    cache: HashMap<String, Arc<Resource>>,
}
struct Resource {
    definitions: Snapshot,
    target: Snapshot,
}

impl Document<'_> {
    pub(crate) fn import_external(&mut self, opt: &crate::Options) {
        let base = opt
            .resources_dir
            .as_ref()
            .and_then(|p| Url::from_directory_path(p).ok());
        Session::default().expand(self, base.as_ref(), 0);
        super::super::cycles::fix_resource_cycles(self);
    }
}

impl Session {
    fn expand(&mut self, doc: &mut Document, base: Option<&Url>, depth: usize) {
        if depth >= 32 {
            return;
        }
        let uses: Vec<_> = doc
            .descendants()
            .filter(|n| n.tag_name() == Some(EId::Use))
            .filter_map(|n| n.attribute::<&str>(AId::Href).map(|h| (n.id, h.to_owned())))
            .collect();
        // Images retain the resource base even after their nodes are imported.
        for attr in &mut doc.attrs {
            if attr.name == AId::Href && !attr.value.starts_with('#') {
                if let Some(uri) = resolve(&attr.value, base) {
                    let text = if uri.fragment().is_none() {
                        uri.to_file_path()
                            .ok()
                            .map(|p| p.to_string_lossy().into_owned())
                            .unwrap_or(uri.to_string())
                    } else {
                        uri.to_string()
                    };
                    attr.value = roxmltree::StringStorage::new_owned(&text);
                }
            }
        }
        for (parent, href) in uses {
            if href.starts_with('#') || doc.get(parent).has_children() {
                continue;
            }
            let Some(uri) = resolve(&href, base) else {
                continue;
            };
            let Some(resource) = self.load(uri, depth + 1) else {
                continue;
            };
            if doc.nodes.len() + resource.definitions.count() + resource.target.count() > 1_000_000
            {
                continue;
            }
            let prefix = format!("__kamin_resource_{}_", doc.nodes.len());
            let empty = doc.attrs.len() as u32;
            let defs = doc.append(
                doc.root_element().id,
                NodeKind::Element {
                    tag_name: EId::Defs,
                    attributes: ShortRange::new(empty, empty),
                },
            );
            resource.definitions.append(doc, defs, &prefix, true);
            resource.target.append(doc, parent, &prefix, false);
        }
    }

    fn load(&mut self, uri: Url, depth: usize) -> Option<Arc<Resource>> {
        let fragment = uri.fragment().unwrap_or("").to_owned();
        let path = std::fs::canonicalize(uri.to_file_path().ok()?).ok()?;
        let mut canonical = Url::from_file_path(&path).ok()?;
        canonical.set_fragment(Some(&fragment));
        let key = canonical.to_string();
        if let Some(resource) = self.cache.get(&key) {
            return Some(resource.clone());
        }
        if depth >= 32 || !self.active.insert(key.clone()) {
            return None;
        }
        let result = (|| {
            let bytes = read_resource(&path)?;
            let text = std::str::from_utf8(&bytes).ok()?;
            let xml = roxmltree::Document::parse_with_options(
                text,
                roxmltree::ParsingOptions {
                    allow_dtd: true,
                    ..Default::default()
                },
            )
            .ok()?;
            let mut source = parse(&xml, None, true).ok()?;
            self.expand(&mut source, Some(&canonical), depth);
            let target = if fragment.is_empty() {
                source.root_element()
            } else {
                source.element_by_id(&fragment)?
            };
            Some(Arc::new(Resource {
                target: Snapshot::capture(target, 0)?,
                definitions: Snapshot::capture(source.root(), 0)?,
            }))
        })();
        self.active.remove(&key);
        if let Some(resource) = &result {
            self.cache.insert(key, resource.clone());
        }
        result
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

fn read_resource(path: &std::path::Path) -> Option<Vec<u8>> {
    let file = std::fs::File::open(path).ok()?;
    if file.metadata().ok()?.len() > RESOURCE_BYTE_LIMIT {
        return None;
    }
    read_bounded(file, RESOURCE_BYTE_LIMIT)
}

fn read_bounded(reader: impl std::io::Read, limit: u64) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut bytes = Vec::new();
    reader.take(limit + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= limit).then_some(bytes)
}

#[cfg(test)]
mod byte_limit_tests {
    use super::read_bounded;
    use std::cell::Cell;
    use std::io::{self, Read};

    #[test]
    fn accepts_the_limit_and_rejects_one_extra_byte() {
        assert_eq!(read_bounded(&b"abcd"[..], 4), Some(b"abcd".to_vec()));
        assert!(read_bounded(&b"abcde"[..], 4).is_none());
    }

    #[test]
    fn growing_source_is_never_read_past_limit_plus_one() {
        struct Growing<'a>(&'a Cell<usize>);
        impl Read for Growing<'_> {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                buf.fill(b'x');
                self.0.set(self.0.get() + buf.len());
                Ok(buf.len())
            }
        }
        let count = Cell::new(0);
        assert!(read_bounded(Growing(&count), 4).is_none());
        assert_eq!(count.get(), 5);
    }
}
