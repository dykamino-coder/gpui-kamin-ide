//! Проверки снимков реестра, первого доступного семейства и ревизий загрузки.

use super::*;
use crate::document::state::DocState;
use crate::font_facts::{FontFactsError, FontInstanceFacts, FontRequest};
use crate::style::{computed::Computed, css::parse_decls};

fn document() -> Rc<DocState> {
    let fonts = FontContext {
        installed: Rc::new(
            ["ahem", "consolas", "ms gothic", "times new roman"]
                .map(str::to_owned)
                .into_iter()
                .collect(),
        ),
        ..Default::default()
    };
    let doc = DocState::new(Default::default());
    fonts.install(doc.values());
    doc
}

fn family(doc: &DocState, value: &str) -> Computed {
    let mut style = Computed::default();
    style.apply_decls(&parse_decls(&format!("font-family: {value}")), doc.values());
    style
}

#[test]
fn installed_alias_generic_mono_and_language_keep_the_existing_order() {
    let doc = document();
    assert_eq!(
        family(&doc, "Absent, aHeM").font_family.as_deref(),
        Some("aHeM")
    );
    assert_eq!(
        family(&doc, "Absent, serif").font_family.as_deref(),
        Some("Times New Roman")
    );
    assert_eq!(family(&doc, "Absent").font_family.as_deref(), Some(""));
    let mono = family(&doc, "monospace");
    assert_eq!(mono.monospace, Some(true));
    assert_eq!(
        mono.font_families.as_deref(),
        Some(["Consolas".into()].as_slice())
    );
    let snapshot = doc.font_snapshot();
    assert_eq!(snapshot.mono_family(None), "Consolas");
    assert_eq!(snapshot.mono_family(Some("JA-jp")), "MS Gothic");
    assert_eq!(snapshot.mono_family(Some("en")), "Consolas");
    // M preserves the old named-family-first branch; changing list order is S.
    assert_eq!(
        family(&doc, "serif, Ahem").font_family.as_deref(),
        Some("Ahem")
    );
}

#[test]
fn immutable_registry_cold_warm_loading_and_two_documents() {
    let a = document();
    let b = document();
    let old = a.font_snapshot();
    assert_eq!(old, a.font_snapshot());
    assert!(Rc::ptr_eq(&old.registry, &a.font_snapshot().registry));
    assert_ne!(old.environment(), b.font_snapshot().environment());
    let calls = Rc::new(std::cell::Cell::new(0));
    let count = calls.clone();
    a.metrics
        .font_context()
        .with_loader(move |_| {
            count.set(count.get() + 1);
            Some("Ahem".into())
        })
        .install(a.values());
    // A valid empty sfnt directory is sufficient for the injected loader.
    let css = "@font-face { font-family: Alias; \
        src: url(data:font/ttf;base64,AAEAAAAAAAAAAAAA); unicode-range: U+41-5A }";
    crate::text::fonts::load_faces(css, a.values()).unwrap();
    let cold = a.font_snapshot();
    assert!(cold.generation() > old.generation());
    assert_eq!(old.alias("Alias"), None);
    assert_eq!(cold.alias("aLIas").as_deref(), Some("Ahem"));
    assert!(!cold.covers_space("Alias"));
    assert_eq!(
        family(&a, "Alias, Ahem").font_family.as_deref(),
        Some("Ahem")
    );
    assert!(cold.covers_space("Ahem"));
    assert_eq!(b.font_snapshot().alias("Alias"), None);
    crate::text::fonts::load_faces(css, a.values()).unwrap();
    let warm = a.font_snapshot();
    assert!(warm.generation() > cold.generation());
    assert_eq!(warm.registry, cold.registry);
    assert_eq!(calls.get(), 1);
    let with_space = css.replace("U+41-5A", "U+20-7F");
    crate::text::fonts::load_faces(&with_space, a.values()).unwrap();
    let later = a.font_snapshot();
    assert!(later.covers_space("Alias"));
    assert!(!cold.covers_space("Alias"));
    assert_eq!(
        family(&a, "Alias, Ahem").font_family.as_deref(),
        Some("Alias")
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(
        a.font_provider()
            .select(FontRequest::new("Alias", 16.0), &cold),
        Err(FontFactsError::StaleSnapshot)
    );
}

#[test]
fn backend_face_identity_is_native_document_scoped_and_revision_scoped() {
    let fonts = FontContext::new(Arc::new(gpui::TextSystem::new(Arc::new(
        gpui::NoopTextSystem::new(),
    ))));
    let a = DocState::new(Default::default());
    let b = DocState::new(Default::default());
    fonts.install(a.values());
    fonts.install(b.values());
    let request = FontRequest::new("Ahem", 16.0);
    let first = a
        .font_provider()
        .select(request, &a.font_snapshot())
        .unwrap();
    let warm = a
        .font_provider()
        .select(request, &a.font_snapshot())
        .unwrap();
    assert_eq!(first, warm);
    assert_eq!(first.face.backend_key, 1);
    assert_eq!(first.units_per_em, 1000);
    assert_eq!(first.instance, FontInstanceFacts::Absent);
    let other = b
        .font_provider()
        .select(request, &b.font_snapshot())
        .unwrap();
    assert_ne!(first.face, other.face);
    let mut unsupported = request;
    let variations = [("wght".into(), 700.0)];
    unsupported.variations = &variations;
    unsupported.optical_size = Some(16.0);
    unsupported.script = Some("Latn");
    let unsupported = a
        .font_provider()
        .select(unsupported, &a.font_snapshot())
        .unwrap();
    assert_eq!(unsupported.face, first.face);
    assert_eq!(
        unsupported.instance,
        FontInstanceFacts::Unsupported {
            variations: true,
            optical_size: true,
            script: true,
        }
    );
    crate::text::fonts::load_faces("", a.values()).unwrap();
    let later = a
        .font_provider()
        .select(request, &a.font_snapshot())
        .unwrap();
    assert_ne!(first.face, later.face);
}
