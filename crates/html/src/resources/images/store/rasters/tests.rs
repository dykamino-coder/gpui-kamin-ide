//! Размер, плотность, вид, шрифты и внешние зависимости в кэше SVG документа.

use super::*;
use crate::resources::images::store::{ImageServices, ResourcePolicy};
use crate::resources::images::{cache::DecodedImageCache, fixtures, tests_support::TempDir};
use crate::resources::loader::{LoadError, Loaded, ResourceLoader};
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

const MARKUP: &str = "<svg xmlns='http://www.w3.org/2000/svg' width='10' height='8'><rect width='10' height='8' fill='green'/></svg>";

#[test]
fn concrete_viewport_handles_invalid_intrinsic_sizes_and_disabled_rendering() {
    let store = ImageStore::default();
    let request = RasterRequest::new(10.0, 10.0, 1.0, 0.0);
    for dimensions in [
        "width='0' height='50'",
        "width='-1' height='50'",
        "width='-1' height='-50'",
    ] {
        let markup = format!(
            "<svg xmlns='http://www.w3.org/2000/svg' {dimensions} viewBox='0 0 1 1'><rect width='1' height='1' fill='red'/></svg>"
        );
        let viewport = crate::resources::images::viewport::with_viewport(&markup, (10.0, 10.0));
        let raster = store.rasterize_svg(&viewport, request).unwrap();
        assert!(raster.bytes.chunks_exact(4).all(|p| p == [0, 0, 255, 255]));
    }
    let markup = "<svg xmlns='http://www.w3.org/2000/svg' width='10' height='10' viewBox='0 0 0 10'><rect width='10' height='10' fill='red'/></svg>";
    let raster = store.rasterize_svg(markup, request).unwrap();
    assert!(raster.bytes.iter().all(|b| *b == 0));
}

#[test]
fn cache_uses_actual_device_extent_density_view_and_document() {
    let store = ImageStore::default();
    let req = RasterRequest::new(10.1, 8.0, 2.0, 0.0);
    let a = store.rasterize_svg(MARKUP, req).unwrap();
    assert!(Arc::ptr_eq(&a, &store.rasterize_svg(MARKUP, req).unwrap()));
    let b = store
        .rasterize_svg(MARKUP, RasterRequest::new(10.4, 8.0, 2.0, 0.0))
        .unwrap();
    assert_eq!((a.width, b.width), (20, 21));
    let c = store
        .rasterize_svg(MARKUP, RasterRequest::new(20.2, 16.0, 1.0, 0.0))
        .unwrap();
    assert!(!Arc::ptr_eq(&a, &c));
    let view = MARKUP.replace(
        "width='10' height='8'",
        "width='10' height='8' viewBox='0 0 5 4'",
    );
    assert!(!Arc::ptr_eq(&a, &store.rasterize_svg(&view, req).unwrap()));
    let other = ImageStore::default();
    assert!(!Arc::ptr_eq(&a, &other.rasterize_svg(MARKUP, req).unwrap()));
}

#[test]
fn injected_font_database_changes_pixels_and_invalidates_rasters() {
    let mut store = ImageStore::default();
    let req = RasterRequest::new(80.0, 30.0, 1.0, 0.0);
    let mut fonts = Database::new();
    fonts.load_font_data(
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../shell/assets/fonts/JetBrainsMono-Variable.ttf"
        ))
        .to_vec(),
    );
    let family = fonts.faces().next().unwrap().families[0].0.clone();
    let markup = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='80' height='30'><text x='0' y='22' font-family='{family}' font-size='20'>SVG</text></svg>"
    );
    let empty = store.rasterize_svg(&markup, req).unwrap();
    assert!(empty.bytes.chunks_exact(4).all(|p| p[3] == 0));
    let fonts = Arc::new(fonts);
    store.set_svg_fonts(fonts.clone());
    let text = store.rasterize_svg(&markup, req).unwrap();
    assert!(text.bytes.chunks_exact(4).any(|p| p[3] > 0));
    store.set_svg_fonts(fonts);
    assert!(Arc::ptr_eq(
        &text,
        &store.rasterize_svg(&markup, req).unwrap()
    ));
    store.begin_generation();
    assert!(!Arc::ptr_eq(
        &text,
        &store.rasterize_svg(&markup, req).unwrap()
    ));
}

struct Loader {
    bytes: Mutex<Vec<u8>>,
    revision: AtomicUsize,
    reads: AtomicUsize,
}
impl ResourceLoader for Loader {
    fn revision(&self, _: &ResourceUrl) -> Revision {
        Revision::File {
            len: self.revision.load(Ordering::Relaxed) as u64,
            modified: None,
        }
    }
    fn load(&self, url: &ResourceUrl) -> Result<Loaded, LoadError> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        Ok(Loaded {
            bytes: self.bytes.lock().unwrap().clone().into(),
            revision: self.revision(url),
        })
    }
}

#[test]
fn external_bytes_are_cached_revalidated_and_checked_by_document_policy() {
    let loader = Arc::new(Loader {
        bytes: Mutex::new(fixtures::solid_png([255, 0, 0, 255])),
        revision: AtomicUsize::new(1),
        reads: AtomicUsize::new(0),
    });
    let services = ImageServices {
        loader: loader.clone(),
        cache: Arc::new(DecodedImageCache::new(1024 * 1024)),
    };
    let store = ImageStore::new(services.clone(), None, ResourcePolicy::default());
    let markup = "<svg xmlns='http://www.w3.org/2000/svg' width='1' height='1'><image href='file:///resource.png' width='1' height='1'/></svg>";
    let req = RasterRequest::new(1.0, 1.0, 1.0, 0.0);
    let a = store.rasterize_svg(markup, req).unwrap();
    assert_eq!(a.pixel(0, 0), Some([0, 0, 255, 255]));
    assert!(Arc::ptr_eq(&a, &store.rasterize_svg(markup, req).unwrap()));
    assert_eq!(loader.reads.load(Ordering::Relaxed), 1);
    // SVG-картинка использует secure static mode даже при доступном файле.
    let secure = store
        .rasterize_svg(markup, req.without_external_resources())
        .unwrap();
    assert_eq!(secure.pixel(0, 0), Some([0, 0, 0, 0]));
    assert_eq!(loader.reads.load(Ordering::Relaxed), 1);
    assert!(!Arc::ptr_eq(&a, &secure));
    use base64::Engine;
    let data =
        base64::engine::general_purpose::STANDARD.encode(fixtures::solid_png([255, 0, 0, 255]));
    let inline = markup.replace(
        "file:///resource.png",
        &format!("data:image/png;base64,{data}"),
    );
    assert_eq!(
        store
            .rasterize_svg(&inline, req.without_external_resources())
            .unwrap()
            .pixel(0, 0),
        Some([0, 0, 255, 255])
    );
    *loader.bytes.lock().unwrap() = fixtures::solid_png([0, 0, 255, 255]);
    loader.revision.store(2, Ordering::Relaxed);
    let b = store.rasterize_svg(markup, req).unwrap();
    assert_eq!(b.pixel(0, 0), Some([255, 0, 0, 255]));
    let denied = ImageStore::new(services, None, ResourcePolicy::data_only());
    assert_eq!(
        denied.rasterize_svg(markup, req).unwrap().pixel(0, 0),
        Some([0, 0, 0, 0])
    );
    assert_eq!(loader.reads.load(Ordering::Relaxed), 2);
}

#[test]
fn svg_links_resolve_from_resource_base_and_file_url_decodes_spaces() {
    // RFC 8089 §§2,4: процентное кодирование file: — синтаксис URI, а не имя файла.
    let mut temp = TempDir::new("svg-external-spaces");
    let path = temp.write("red square.png", &fixtures::solid_png([255, 0, 0, 255]));
    let base = url::Url::from_file_path(temp.path().join("image.svg")).unwrap();
    let markup = "<svg xmlns='http://www.w3.org/2000/svg' width='1' height='1'><image href='red%20square.png' width='1' height='1'/></svg>";
    let linked = svg_links::with_base(markup, Some(&base));
    assert!(linked.contains(url::Url::from_file_path(path).unwrap().as_str()));
    let store = ImageStore::default();
    assert_eq!(
        store
            .rasterize_svg(&linked, RasterRequest::new(1.0, 1.0, 1.0, 0.0))
            .unwrap()
            .pixel(0, 0),
        Some([0, 0, 255, 255])
    );
}

#[test]
fn raw_markup_does_not_implicitly_inherit_document_base() {
    let mut temp = TempDir::new("svg-explicit-base");
    let name = format!("imgsvc-markup-base-only-{}.png", std::process::id());
    temp.write(&name, &fixtures::solid_png([255, 0, 0, 255]));
    let base = url::Url::from_file_path(temp.path().join("document.html")).unwrap();
    let store = ImageStore::new(
        ImageServices::process(),
        Some(base.clone()),
        ResourcePolicy::default(),
    );
    let markup = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='1' height='1'><image href='{name}' width='1' height='1'/></svg>"
    );
    let req = RasterRequest::new(1.0, 1.0, 1.0, 0.0);
    assert_eq!(
        store.rasterize_svg(&markup, req).unwrap().pixel(0, 0),
        Some([0, 0, 0, 0])
    );
    let linked = svg_links::with_base(&markup, Some(&base));
    assert_eq!(
        store.rasterize_svg(&linked, req).unwrap().pixel(0, 0),
        Some([0, 0, 255, 255])
    );
}
