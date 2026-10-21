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
}

impl<'a> DocumentBuilder<'a> {
    pub(super) fn new(html: &'a str, theme_css: &'a str) -> Self {
        Self {
            html,
            theme_css,
            base_url: None,
            xml: false,
            media: Media::default(),
        }
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
    pub fn build(&self) -> Document {
        Document::build(
            self.html,
            self.theme_css,
            self.base_url.clone(),
            self.xml,
            self.media,
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
    fn builder_matches_compatible_constructors() {
        let html = "<!doctype html><style>@media print {p {width:17px}}</style><p>x</p>";
        let css = "p {height:11px}";
        same(
            &Document::new(html, css),
            &Document::builder(html, css).build(),
        );
        for print in [false, true] {
            same(
                &Document::new_media(html, css, print),
                &Document::builder(html, css).print(print).build(),
            );
            for path in [Path::new("source.html"), Path::new("source.SVG")] {
                same(
                    &Document::new_with_path_media(html, css, path, print),
                    &Document::builder(html, css).path(path).print(print).build(),
                );
            }
        }
        for path in [Path::new("source.html"), Path::new("source.SVG")] {
            same(
                &Document::new_with_path(html, css, path),
                &Document::builder(html, css).path(path).build(),
            );
        }
        for uri in ["file:///tests/source.html", "invalid url"] {
            same(
                &Document::new_with_base(html, css, uri),
                &Document::builder(html, css).base_uri(uri).build(),
            );
        }
    }

    #[test]
    fn reusable_builder_keeps_independent_documents_and_update_options() {
        let html = "<svg xmlns='http://www.w3.org/2000/svg'><g/></svg>";
        let mut builder = Document::builder(html, "");
        builder.path(Path::new("source.SVG")).print(true);
        let mut a = builder.build();
        let b = builder.build();
        same(&a, &b);
        assert!(!Rc::ptr_eq(&a.state, &b.state));
        assert!(a.xml && a.is_print());
        let new = "<svg xmlns='http://www.w3.org/2000/svg'><metadata/></svg>";
        assert!(a.update(new, ""));
        same(
            &a,
            &Document::builder(new, "")
                .path(Path::new("source.SVG"))
                .print(true)
                .build(),
        );
        builder.base_uri("invalid url");
        let html_doc = builder.build();
        assert!(!html_doc.xml);
        assert!(html_doc.base_url.is_none());
        assert!(html_doc.is_print());
    }
}
