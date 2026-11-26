//! Local path admission and deterministic, non-symlink-recursive folder discovery.

use std::{
    fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};

pub fn is_page(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            ["html", "htm", "xhtml", "svg"]
                .iter()
                .any(|candidate| ext.eq_ignore_ascii_case(candidate))
        })
}

pub fn absolute(path: &Path) -> io::Result<PathBuf> {
    std::path::absolute(path)
}

pub fn list(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut pending = vec![root.to_owned()];
    let mut pages = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() && !kind.is_symlink() {
                pending.push(entry.path());
            } else if kind.is_file() && is_page(&entry.path()) {
                pages.push(entry.path());
            }
        }
    }
    pages.sort_by(|a, b| {
        a.strip_prefix(root)
            .unwrap_or(a)
            .cmp(b.strip_prefix(root).unwrap_or(b))
    });
    Ok(pages)
}

pub fn stamp(path: &Path) -> Option<(SystemTime, u64)> {
    let meta = fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn admits_only_page_extensions_case_insensitively() {
        for path in ["a.html", "b.HTM", "c.Xhtml", "d.SVG"] {
            assert!(is_page(Path::new(path)));
        }
        for path in ["html", "a.html.txt", "a.css", "a.png"] {
            assert!(!is_page(Path::new(path)));
        }
    }

    #[test]
    fn relative_and_absolute_paths_preserve_spaces_and_unicode() {
        let relative = Path::new("pages with spaces/\u{044f}.html");
        let resolved = absolute(relative).unwrap();
        assert!(resolved.is_absolute());
        assert!(resolved.ends_with(relative));
        assert_eq!(absolute(&resolved).unwrap(), resolved);
    }

    #[test]
    fn recursive_listing_is_sorted_and_filters_non_pages() {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "kamin-viewer-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("nested")).unwrap();
        for file in ["z.html", "a.HTM", "nested/c.svg", "nested/b.xhtml", "x.css"] {
            fs::write(root.join(file), "").unwrap();
        }
        let relative: Vec<_> = list(&root)
            .unwrap()
            .into_iter()
            .map(|p| p.strip_prefix(&root).unwrap().to_owned())
            .collect();
        assert_eq!(
            relative,
            ["a.HTM", "nested/b.xhtml", "nested/c.svg", "z.html"].map(PathBuf::from)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_folder_and_file_are_reported() {
        let path = std::env::temp_dir().join("kamin-viewer-nonexistent-folder");
        assert!(list(&path).is_err());
        assert_eq!(stamp(&path), None);
    }
}
