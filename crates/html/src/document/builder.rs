//! Настройки разбора Document: источник ресурсов, формат и носитель.

use super::Document;
use crate::style::css::Media;
use std::path::Path;
use url::Url;

/// Настройки документа до разбора. Повторная сборка создаёт независимое состояние.
///
/// `path` и `base_uri` заменяют источник целиком: последний вызов определяет
/// базовый URL и формат (XML для SVG-файла, HTML для URL).
pub struct DocumentBuilder<'a> {
    html: &'a str,
    theme_css: &'a str,
    base_url: Option<Url>,
    xml: bool,
    media: Media,
    fonts: crate::text::metrics::FontContext,
    limits: crate::budget::DocumentLimits,
    governor: crate::budget::ResourceGovernor,
}

impl<'a> DocumentBuilder<'a> {
    pub(super) fn new(html: &'a str, theme_css: &'a str) -> Self {
        Self {
            html,
            theme_css,
            base_url: None,
            xml: false,
            media: Media::default(),
            fonts: Default::default(),
            limits: Default::default(),
            governor: Default::default(),
        }
    }

    /// Передать платформенные шрифты до разбора и измерения CSS.
    pub fn fonts(&mut self, fonts: &crate::text::metrics::FontContext) -> &mut Self {
        self.fonts = fonts.clone();
        self
    }

    /// Immutable profile документа; host может только уменьшать defaults.
    pub fn limits(&mut self, limits: crate::budget::DocumentLimits) -> &mut Self {
        self.limits = limits;
        self
    }

    /// Один экземпляр хоста разделяется всеми его документами и cache/pool.
    pub fn governor(&mut self, governor: &crate::budget::ResourceGovernor) -> &mut Self {
        self.governor = governor.clone();
        self
    }

    /// Выбрать печатный носитель (иначе экранный).
    pub fn print(&mut self, print: bool) -> &mut Self {
        self.media = Media::for_medium(print);
        self
    }

    /// Задать URL источника HTML. Невалидный URL оставляет документ без базы.
    pub fn base_uri(&mut self, base_uri: &str) -> &mut Self {
        self.base_url = Url::parse(base_uri).ok();
        self.xml = false;
        self
    }

    /// Задать локальный источник; расширение SVG без учёта регистра выбирает XML.
    pub fn path(&mut self, path: &Path) -> &mut Self {
        self.base_url = std::path::absolute(path)
            .ok()
            .and_then(|p| Url::from_file_path(p).ok());
        self.xml = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("svg"));
        self
    }

    /// Разобрать документ с выбранными настройками в свежем состоянии.
    ///
    /// Исходный DOM сохраняется целиком; предел обхода даёт Degraded в report().
    /// Source admission предшествует разбору и ранним проходам документа.
    pub fn build(&self) -> Result<Document, super::DocumentFailure> {
        let budget = crate::budget::DocumentBudget::new(self.limits.clone(), self.governor.clone())
            .map_err(|e| super::DocumentFailure::new(super::FailureReason::Budget(e)))?;
        let revision = budget.revision();
        revision
            .source(self.html, self.theme_css)
            .map_err(|e| super::DocumentFailure::new(super::FailureReason::Budget(e)))?;
        Document::build(
            self.html,
            self.theme_css,
            self.base_url.clone(),
            self.xml,
            self.media,
            &self.fonts,
            revision,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    fn same(a: &Document, b: &Document) {
        assert_eq!(a.base_url, b.base_url);
        assert_eq!(a.xml, b.xml);
        assert_eq!(a.salt(), b.salt());
        assert_eq!(a.is_print(), b.is_print());
        assert_eq!(format!("{:?}", a.nodes()), format!("{:?}", b.nodes()));
        assert_eq!(
            format!("{:?}", a.root_style()),
            format!("{:?}", b.root_style())
        );
    }

    #[test]
    fn builder_selects_source_format_and_medium() {
        let html = "<p>x</p>";
        let screen = Document::builder(html, "").build().unwrap();
        assert!(screen.base_url.is_none());
        assert!(!screen.xml && !screen.is_print());
        for print in [false, true] {
            for path in [Path::new("source.html"), Path::new("source.SVG")] {
                let doc = Document::builder(html, "")
                    .path(path)
                    .print(print)
                    .build()
                    .unwrap();
                assert_eq!(doc.is_print(), print);
                assert_eq!(doc.xml, path.extension().unwrap() == "SVG");
                assert_eq!(
                    doc.base_url,
                    Url::from_file_path(std::path::absolute(path).unwrap()).ok()
                );
            }
        }
        let base = Document::builder(html, "")
            .base_uri("file:///tests/source.html")
            .build()
            .unwrap();
        assert_eq!(base.base_url, Url::parse("file:///tests/source.html").ok());
        assert!(!base.xml && !base.is_print());
        let invalid = Document::builder(html, "")
            .base_uri("invalid url")
            .build()
            .unwrap();
        assert!(invalid.base_url.is_none());
        assert!(!invalid.xml && !invalid.is_print());
    }

    #[test]
    fn reusable_builder_keeps_independent_documents_and_update_options() {
        let html = "<svg xmlns='http://www.w3.org/2000/svg'><g/></svg>";
        let mut builder = Document::builder(html, "");
        builder.path(Path::new("source.SVG")).print(true);
        let mut a = builder.build().unwrap();
        let b = builder.build().unwrap();
        same(&a, &b);
        assert!(!Rc::ptr_eq(&a.state, &b.state));
        assert!(a.xml && a.is_print());
        let new = "<svg xmlns='http://www.w3.org/2000/svg'><metadata/></svg>";
        assert!(a.update(new, "").unwrap().changed());
        same(
            &a,
            &Document::builder(new, "")
                .path(Path::new("source.SVG"))
                .print(true)
                .build()
                .unwrap(),
        );
        builder.base_uri("invalid url");
        let html_doc = builder.build().unwrap();
        assert!(!html_doc.xml);
        assert!(html_doc.base_url.is_none());
        assert!(html_doc.is_print());
    }
}
