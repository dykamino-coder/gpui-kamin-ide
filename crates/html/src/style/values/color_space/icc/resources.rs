//! Load named ICC resources against the effective document base URL.

use super::PROFILES;

pub fn load_profiles(css: &str) {
    load_profiles_with_base(css, None);
}

pub(crate) fn load_profiles_with_base(css: &str, base: Option<&url::Url>) {
    PROFILES.with(|p| p.borrow_mut().clear());
    let css = &crate::style::css::strip_comments(css);
    let lower = css.to_ascii_lowercase();
    let mut from = 0usize;
    while let Some(at) = lower[from..].find("@color-profile") {
        let start = from + at;
        let Some(open) = css[start..].find('{') else {
            break;
        };
        let name = css[start + "@color-profile".len()..start + open]
            .trim()
            .to_string();
        if !name.starts_with("--") || open > 64 {
            from = start + "@color-profile".len();
            continue;
        }
        let Some(close) = css[start + open..].find('}') else {
            break;
        };
        let block = &css[start + open + 1..start + open + close];
        from = start + open + close;
        let Some(src) = block.split(';').find_map(|d| {
            let (k, v) = d.split_once(':')?;
            k.trim()
                .eq_ignore_ascii_case("src")
                .then(|| v.trim().to_string())
        }) else {
            continue;
        };
        let Some(path) = crate::style::computed::parse_url(&src) else {
            continue;
        };
        let path = crate::document::resource_url::resolve(&path, base);
        if let Some(bytes) = crate::document::resource_url::read(&path) {
            PROFILES.with(|p| p.borrow_mut().insert(name, bytes));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icc_resources_resolve_relative_urls_and_decode_file_paths() {
        let path = std::env::temp_dir().join(format!(
            "kamin-profile-{}-тест space.icc",
            std::process::id()
        ));
        std::fs::write(&path, b"profile").unwrap();
        let base = url::Url::from_file_path(path.parent().unwrap().join("page.html")).unwrap();
        let css = format!(
            "@color-profile --base-test {{src: url('{}')}}",
            path.file_name().unwrap().to_str().unwrap()
        );
        load_profiles_with_base(&css, Some(&base));
        PROFILES.with(|profiles| {
            assert_eq!(
                profiles.borrow().get("--base-test").map(Vec::as_slice),
                Some(b"profile".as_slice())
            )
        });
        load_profiles("");
        std::fs::remove_file(path).unwrap();
    }
}
