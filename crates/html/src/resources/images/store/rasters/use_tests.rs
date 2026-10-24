//! Внешние use: запреты до чтения, базы ресурсов, вложенные ревизии и fallback.
use super::*;
use crate::resources::images::store::{ImageServices, ResourcePolicy};
use crate::resources::images::{cache::DecodedImageCache, svg_links, tests_support::TempDir};
use crate::resources::loader::{LoadError, Loaded, ResourceLoader, SyncLoader};
use std::collections::HashMap;

#[derive(Default)]
struct Loader {
    resources: Mutex<HashMap<String, Vec<u8>>>,
    reads: Mutex<Vec<String>>,
    files: bool,
}
impl Loader {
    fn put(&self, url: &str, text: &str) {
        self.resources
            .lock()
            .unwrap()
            .insert(url.into(), text.as_bytes().to_vec());
    }
    fn store(self: &Arc<Self>, policy: ResourcePolicy) -> ImageStore {
        ImageStore::new(
            ImageServices {
                loader: self.clone(),
                cache: Arc::new(DecodedImageCache::new(MIB)),
            },
            None,
            policy,
        )
    }
}
impl ResourceLoader for Loader {
    fn revision(&self, url: &ResourceUrl) -> Revision {
        if self.files {
            return SyncLoader.revision(url);
        }
        self.resources
            .lock()
            .unwrap()
            .get(url.as_str())
            .map_or(Revision::Absent, |bytes| Revision::File {
                len: bytes.len() as u64,
                modified: None,
            })
    }
    fn load(&self, url: &ResourceUrl) -> Result<Loaded, LoadError> {
        self.reads.lock().unwrap().push(url.as_str().into());
        if self.files {
            return SyncLoader.load(url);
        }
        let bytes = self
            .resources
            .lock()
            .unwrap()
            .get(url.as_str())
            .cloned()
            .ok_or(LoadError::NotFound)?;
        Ok(Loaded {
            bytes: bytes.into(),
            revision: self.revision(url),
        })
    }
}
fn drawing(body: &str) -> String {
    format!("<svg xmlns='http://www.w3.org/2000/svg' width='2' height='1'>{body}</svg>")
}
fn target(fill: &str) -> String {
    drawing(&format!(
        "<rect id='r' width='2' height='1' fill='{fill}'/>"
    ))
}
fn request() -> RasterRequest {
    RasterRequest::new(2.0, 1.0, 1.0, 0.0)
}

#[test]
fn data_url_use_is_invalid_before_the_document_loader() {
    let loader = Arc::new(Loader::default());
    let href = "data:image/svg+xml,%3Csvg%2F%3E";
    loader.put(href, &target("red"));
    let store = loader.store(ResourcePolicy::default());
    let markup = drawing(&format!(
        "<rect width='2' height='1' fill='lime'/><use href='{href}#r'/>"
    ));
    assert_eq!(
        store.rasterize_svg(&markup, request()).unwrap().pixel(0, 0),
        Some([0, 255, 0, 255])
    );
    assert!(loader.reads.lock().unwrap().is_empty());
}

#[test]
fn policy_and_secure_request_prevent_reading_existing_use_file() {
    let mut temp = TempDir::new("use-policy");
    let path = temp.write("secret.svg", target("red").as_bytes());
    let href = url::Url::from_file_path(path).unwrap();
    let markup = drawing(&format!("<use href='{href}#r'/>"));
    let loader = Arc::new(Loader {
        files: true,
        ..Loader::default()
    });
    let denied = loader.store(ResourcePolicy::data_only());
    assert!(
        denied
            .rasterize_svg(&markup, request())
            .unwrap()
            .bytes
            .iter()
            .all(|b| *b == 0)
    );
    let allowed = loader.store(ResourcePolicy::default());
    assert!(
        allowed
            .rasterize_svg(&markup, request().without_external_resources())
            .unwrap()
            .bytes
            .iter()
            .all(|b| *b == 0)
    );
    assert!(loader.reads.lock().unwrap().is_empty());
    assert_eq!(
        allowed
            .rasterize_svg(&markup, request())
            .unwrap()
            .pixel(0, 0),
        Some([0, 0, 255, 255])
    );
    assert_eq!(loader.reads.lock().unwrap().len(), 1);
}

#[test]
fn nested_use_revisions_invalidate_raster_and_keep_source_url_bases() {
    let loader = Arc::new(Loader::default());
    loader.put(
        "https://example.test/assets/defs.svg",
        &drawing("<g id='r'><use href='nested/shape.svg#r'/></g>"),
    );
    loader.put(
        "https://example.test/assets/nested/shape.svg",
        &target("red"),
    );
    let store = loader.store(ResourcePolicy::default());
    let markup = svg_links::with_base(
        &drawing("<use href='defs.svg#r'/>"),
        Some(&url::Url::parse("https://example.test/assets/root.svg").unwrap()),
    );
    let red = store.rasterize_svg(&markup, request()).unwrap();
    assert_eq!(red.pixel(0, 0), Some([0, 0, 255, 255]));
    assert!(Arc::ptr_eq(
        &red,
        &store.rasterize_svg(&markup, request()).unwrap()
    ));
    assert_eq!(loader.reads.lock().unwrap().len(), 2);
    loader.put(
        "https://example.test/assets/nested/shape.svg",
        &target("blue"),
    );
    let blue = store.rasterize_svg(&markup, request()).unwrap();
    assert_eq!(blue.pixel(0, 0), Some([255, 0, 0, 255]));
    assert!(!Arc::ptr_eq(&red, &blue));
    assert_eq!(loader.reads.lock().unwrap().len(), 3);
}

#[test]
fn base_change_selects_another_external_use_resource() {
    let loader = Arc::new(Loader::default());
    loader.put("https://example.test/a/defs.svg", &target("red"));
    loader.put("https://example.test/b/defs.svg", &target("blue"));
    let mut store = loader.store(ResourcePolicy::default());
    let markup = format!(
        "<!DOCTYPE svg PUBLIC '-//W3C//DTD SVG 1.1//EN' 'http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd'>{}",
        drawing("<use href='defs.svg#r'/>")
    );
    let mut rasters = Vec::new();
    for dir in ["a", "b"] {
        store.set_base(Some(
            url::Url::parse(&format!("https://example.test/{dir}/document.html")).unwrap(),
        ));
        let (source, _) = store.resolve("root.svg");
        let linked = svg_links::with_base(&markup, source.base_url().as_ref());
        rasters.push(store.rasterize_svg(&linked, request()).unwrap());
    }
    assert_eq!(rasters[0].pixel(0, 0), Some([0, 0, 255, 255]));
    assert_eq!(rasters[1].pixel(0, 0), Some([255, 0, 0, 255]));
}

#[test]
fn rejected_resources_are_transparent_and_missing_resources_can_appear() {
    let loader = Arc::new(Loader::default());
    let store = loader.store(ResourcePolicy::default());
    let markup = drawing(
        "<use href='https://example.test/missing.svg#r'/><rect x='1' width='1' height='1' fill='lime'/>",
    );
    let fallback = store.rasterize_svg(&markup, request()).unwrap();
    assert_eq!(fallback.pixel(0, 0).unwrap()[3], 0);
    assert_eq!(fallback.pixel(1, 0), Some([0, 255, 0, 255]));
    loader.put("https://example.test/missing.svg", "broken XML");
    assert_eq!(
        store
            .rasterize_svg(&markup, request())
            .unwrap()
            .pixel(0, 0)
            .unwrap()[3],
        0
    );
    loader.put("https://example.test/missing.svg", &target("red"));
    assert_eq!(
        store.rasterize_svg(&markup, request()).unwrap().pixel(0, 0),
        Some([0, 0, 255, 255])
    );
}

#[test]
fn nested_svg_image_cannot_escape_secure_processing_with_use() {
    let mut temp = TempDir::new("nested-use-policy");
    let path = temp.write("secret.svg", target("red").as_bytes());
    let href = url::Url::from_file_path(path).unwrap();
    let nested = drawing(&format!("<use href='{href}#r'/>"));
    let data = nested
        .bytes()
        .map(|b| format!("%{b:02X}"))
        .collect::<String>();
    let markup = drawing(&format!(
        "<image href='data:image/svg+xml,{data}' width='2' height='1'/>"
    ));
    let loader = Arc::new(Loader {
        files: true,
        ..Loader::default()
    });
    let store = loader.store(ResourcePolicy::default());
    assert!(
        store
            .rasterize_svg(&markup, request())
            .unwrap()
            .bytes
            .iter()
            .all(|b| *b == 0)
    );
    assert!(loader.reads.lock().unwrap().is_empty());
}
