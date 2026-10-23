//! Разрешение адресов и синхронная загрузка: `data:`, `file:`, локальный путь, отсутствие
//! файла, неизвестная схема и отделение фрагмента от ключа загрузки.

use super::*;

fn temp_file(name: &str, bytes: &[u8]) -> PathBuf {
    let path = std::env::temp_dir().join(format!("kamin-loader-{}-{name}", std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    path
}

#[test]
fn data_urls_decode_base64_and_percent_payloads() {
    let (url, fragment) = ResourceUrl::resolve("data:text/plain;base64,aGk=", None);
    assert_eq!(fragment, None);
    let loaded = SyncLoader.load(&url).unwrap();
    assert_eq!(&*loaded.bytes, b"hi");
    assert_eq!(loaded.revision, Revision::Immutable);
    let (url, _) = ResourceUrl::resolve("data:image/svg+xml,%3Csvg%3E#x", None);
    // The payload of a data URL is not split at '#': it is part of the body.
    assert_eq!(&*SyncLoader.load(&url).unwrap().bytes, b"<svg>#x");
    let (url, _) = ResourceUrl::resolve("data:no-comma", None);
    assert_eq!(SyncLoader.load(&url).unwrap_err(), LoadError::InvalidUrl);
}

#[test]
fn file_urls_resolve_against_the_base_and_decode_paths() {
    let path = temp_file("картинка test.bin", b"bytes");
    let base = Url::from_file_path(path.parent().unwrap().join("page.html")).unwrap();
    let name = path.file_name().unwrap().to_str().unwrap();
    let (url, fragment) = ResourceUrl::resolve(&format!("{name}#view"), Some(&base));
    assert_eq!(fragment.as_deref(), Some("view"));
    assert!(url.as_str().starts_with("file:"));
    assert!(!url.as_str().contains('#'));
    assert_eq!(url.scheme(), "file");
    let loaded = SyncLoader.load(&url).unwrap();
    assert_eq!(&*loaded.bytes, b"bytes");
    assert!(matches!(loaded.revision, Revision::File { len: 5, .. }));
    // A native absolute path stays a path, with or without a base.
    let (native, _) = ResourceUrl::resolve(path.to_str().unwrap(), Some(&base));
    assert_eq!(&*SyncLoader.load(&native).unwrap().bytes, b"bytes");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn missing_files_and_unknown_schemes_are_typed_errors() {
    let path =
        std::env::temp_dir().join(format!("kamin-loader-{}-missing.png", std::process::id()));
    let (url, _) = ResourceUrl::resolve(path.to_str().unwrap(), None);
    assert_eq!(SyncLoader.load(&url).unwrap_err(), LoadError::NotFound);
    assert_eq!(SyncLoader.revision(&url), Revision::Absent);
    let (http, _) = ResourceUrl::resolve("https://example.com/a.png", None);
    assert_eq!(http.scheme(), "https");
    assert_eq!(
        SyncLoader.load(&http).unwrap_err(),
        LoadError::UnsupportedScheme("https".to_string())
    );
}

#[test]
fn different_bases_resolve_one_authored_url_to_different_keys() {
    let a = Url::parse("file:///tests/a/page.html").unwrap();
    let b = Url::parse("file:///tests/b/page.html").unwrap();
    let (left, _) = ResourceUrl::resolve("img.png", Some(&a));
    let (right, _) = ResourceUrl::resolve("img.png", Some(&b));
    assert_ne!(left, right);
    assert_eq!(left.as_str(), "file:///tests/a/img.png");
}
