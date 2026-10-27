//! Разрешает URL шрифтов и ICC-ресурсов относительно действующей базы документа.

use url::Url;

pub(crate) fn resolve(authored: &str, base: Option<&Url>) -> String {
    // Preserve native absolute paths supported by the local document loader.
    if std::path::Path::new(authored).is_absolute() {
        return authored.to_string();
    }
    base.and_then(|base| base.join(authored).ok())
        .or_else(|| Url::parse(authored).ok())
        .map_or_else(|| authored.to_string(), |url| url.to_string())
}

pub(crate) fn read(resource: &str) -> Option<Vec<u8>> {
    use crate::resources::loader::{ResourceLoader, ResourceUrl, SyncLoader};
    let (url, _) = ResourceUrl::resolve(resource, None);
    Some(SyncLoader.load(&url).ok()?.bytes.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_paths_use_the_effective_base() {
        let base = Url::parse("file:///tests/assets/page.html").unwrap();
        assert_eq!(
            resolve("../fonts/a.ttf", Some(&base)),
            "file:///tests/fonts/a.ttf"
        );
        assert_eq!(
            resolve("data:font/ttf;base64,AAAA", Some(&base)),
            "data:font/ttf;base64,AAAA"
        );
    }

    #[test]
    fn file_urls_decode_spaces_and_unicode() {
        let path = std::env::temp_dir().join(format!(
            "kamin-resource-{}-шрифт test.bin",
            std::process::id()
        ));
        std::fs::write(&path, b"resource").unwrap();
        let resource = Url::from_file_path(&path).unwrap().to_string();
        assert!(resource.contains("%20"));
        assert_eq!(read(&resource).as_deref(), Some(b"resource".as_slice()));
        std::fs::remove_file(path).unwrap();
    }
}
