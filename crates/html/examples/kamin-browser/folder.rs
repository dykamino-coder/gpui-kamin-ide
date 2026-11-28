//! Shared folder filtering and labels for the sidebar and page selector.

use crate::app::Browser;
use gpui::{Context, Window};
use std::path::{Path, PathBuf};

pub fn is_base(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("base.html"))
}

pub fn filtered(files: &[PathBuf], base_only: bool) -> Vec<PathBuf> {
    files
        .iter()
        .filter(|path| !base_only || is_base(path))
        .cloned()
        .collect()
}

pub fn label(path: &Path, root: Option<&Path>, base_only: bool) -> String {
    let relative = root
        .and_then(|root| path.strip_prefix(root).ok())
        .unwrap_or(path);
    let shown = if base_only {
        relative
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .or_else(|| root.and_then(Path::file_name).map(Path::new))
            .unwrap_or(relative)
    } else {
        relative
    };
    shown.display().to_string()
}

impl Browser {
    pub fn page_label(&self, path: &Path) -> String {
        label(path, self.folder.as_deref(), self.base_only)
    }

    pub fn toggle_base_only(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.folder.is_none() {
            return;
        }
        self.base_only = !self.base_only;
        self.files = filtered(&self.all_files, self.base_only);
        self.selected = self
            .files
            .iter()
            .position(|path| Some(path) == self.current.as_ref())
            .unwrap_or(0);
        if self.files.get(self.selected) != self.current.as_ref() {
            self.current = self.files.get(self.selected).cloned();
            self.reload(window, cx);
        }
        self.selector_open = false;
        self.list_scroll.scroll_to_item(self.selected);
        self.list_focus.focus(window, cx);
        eprintln!(
            "Folder filter: base_only={} pages={}",
            self.base_only,
            self.files.len()
        );
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_preserves_order_and_case_insensitive_base_names() {
        let files: Vec<_> = ["a/base.html", "a/probe.html", "b/BASE.HTML", "c/index.html"]
            .into_iter()
            .map(PathBuf::from)
            .collect();
        assert_eq!(
            filtered(&files, true),
            vec![files[0].clone(), files[2].clone()]
        );
        assert_eq!(filtered(&files, false), files);
        assert!(!is_base(Path::new("notbase.html")));
    }

    #[test]
    fn base_labels_name_the_folder_and_keep_nested_paths_distinct() {
        let root = Path::new("pages");
        assert_eq!(
            label(Path::new("pages/board/base.html"), Some(root), true),
            "board"
        );
        assert_eq!(
            label(Path::new("pages/base.html"), Some(root), true),
            "pages"
        );
        assert_eq!(
            label(Path::new("pages/board/base.html"), Some(root), false),
            Path::new("board/base.html").display().to_string()
        );
    }
}
